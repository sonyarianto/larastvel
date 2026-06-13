use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use axum::{
    extract::Request,
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Json, Response},
};
use serde_json::json;

use crate::auth::AuthenticatedUser;

/// The result of an authorization check.
#[derive(Debug, Clone, PartialEq)]
pub enum GateCheck {
    /// The action is allowed.
    Allowed,
    /// The action is denied with an optional message.
    Denied(String),
}

impl IntoResponse for GateCheck {
    fn into_response(self) -> Response {
        match self {
            GateCheck::Allowed => (StatusCode::OK).into_response(),
            GateCheck::Denied(msg) => {
                let body = json!({
                    "error": msg,
                    "message": "This action is unauthorized.",
                });
                (StatusCode::FORBIDDEN, Json(body)).into_response()
            }
        }
    }
}

impl GateCheck {
    pub fn is_allowed(&self) -> bool {
        matches!(self, GateCheck::Allowed)
    }

    pub fn is_denied(&self) -> bool {
        matches!(self, GateCheck::Denied(_))
    }

    pub fn message(&self) -> Option<&str> {
        match self {
            GateCheck::Denied(msg) => Some(msg),
            _ => None,
        }
    }
}

/// Callback type for defining a gate.
///
/// Takes a reference to the authenticated user and optionally additional arguments.
pub type GateCallback = Arc<dyn Fn(&AuthenticatedUser, &[String]) -> GateCheck + Send + Sync>;

/// Callback type for `before` hooks (runs before all gates).
pub type BeforeHook =
    Arc<dyn Fn(&AuthenticatedUser, &str, &[String]) -> Option<GateCheck> + Send + Sync>;

/// Callback type for `after` hooks (runs after all gates).
pub type AfterHook =
    Arc<dyn Fn(&AuthenticatedUser, &str, &[String], &GateCheck) -> Option<GateCheck> + Send + Sync>;

/// Central authorization manager.
///
/// Manages gates (closure-based) and policies (class-based authorization rules).
///
/// # Example
///
/// ```ignore
/// use larastvel_core::auth::{Gate, AuthenticatedUser, GateCheck};
///
/// let gate = Gate::new();
/// gate.define("update-post", |user, args| {
///     // In real usage, args[0] would be the post owner ID
///     if args.first().map(|s| s.as_str()) == Some(&user.user_id) {
///         GateCheck::Allowed
///     } else {
///         GateCheck::Denied("You do not own this post.".to_string())
///     }
/// });
///
/// let user = AuthenticatedUser { user_id: "1".to_string(), claims: ... };
/// assert!(gate.allows(&user, "update-post", &["1".to_string()]));
/// ```
pub struct Gate {
    abilities: Arc<Mutex<HashMap<String, GateCallback>>>,
    policies: Arc<Mutex<HashMap<String, Arc<dyn Policy>>>>,
    before_hooks: Arc<Mutex<Vec<BeforeHook>>>,
    after_hooks: Arc<Mutex<Vec<AfterHook>>>,
}

impl std::fmt::Debug for Gate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Gate")
            .field(
                "abilities",
                &self
                    .abilities
                    .lock()
                    .unwrap()
                    .keys()
                    .cloned()
                    .collect::<Vec<_>>(),
            )
            .field(
                "policies",
                &self
                    .policies
                    .lock()
                    .unwrap()
                    .keys()
                    .cloned()
                    .collect::<Vec<_>>(),
            )
            .field(
                "before_hooks_count",
                &self.before_hooks.lock().unwrap().len(),
            )
            .field("after_hooks_count", &self.after_hooks.lock().unwrap().len())
            .finish()
    }
}

impl Clone for Gate {
    fn clone(&self) -> Self {
        Self {
            abilities: Arc::clone(&self.abilities),
            policies: Arc::clone(&self.policies),
            before_hooks: Arc::clone(&self.before_hooks),
            after_hooks: Arc::clone(&self.after_hooks),
        }
    }
}

impl Default for Gate {
    fn default() -> Self {
        Self::new()
    }
}

impl Gate {
    /// Create a new Gate instance.
    pub fn new() -> Self {
        Self {
            abilities: Arc::new(Mutex::new(HashMap::new())),
            policies: Arc::new(Mutex::new(HashMap::new())),
            before_hooks: Arc::new(Mutex::new(Vec::new())),
            after_hooks: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Define a gate ability.
    ///
    /// The callback receives the authenticated user and a slice of string arguments.
    pub fn define<F>(&self, ability: &str, f: F)
    where
        F: Fn(&AuthenticatedUser, &[String]) -> GateCheck + Send + Sync + 'static,
    {
        let mut abilities = self.abilities.lock().unwrap();
        abilities.insert(ability.to_string(), Arc::new(f));
    }

    /// Register a before hook. Runs before all gate checks.
    ///
    /// If the hook returns `Some(GateCheck)`, that result is used immediately
    /// and the normal gate check is skipped.
    pub fn before<F>(&self, f: F)
    where
        F: Fn(&AuthenticatedUser, &str, &[String]) -> Option<GateCheck> + Send + Sync + 'static,
    {
        let mut hooks = self.before_hooks.lock().unwrap();
        hooks.push(Arc::new(f));
    }

    /// Register an after hook. Runs after all gate checks.
    ///
    /// Can override the result of the normal gate check.
    pub fn after<F>(&self, f: F)
    where
        F: Fn(&AuthenticatedUser, &str, &[String], &GateCheck) -> Option<GateCheck>
            + Send
            + Sync
            + 'static,
    {
        let mut hooks = self.after_hooks.lock().unwrap();
        hooks.push(Arc::new(f));
    }

    /// Register a policy class for a resource type.
    pub fn register_policy(&self, resource: &str, policy: Arc<dyn Policy>) {
        let mut policies = self.policies.lock().unwrap();
        policies.insert(resource.to_string(), policy);
    }

    /// Check if a user is allowed to perform an ability.
    pub fn allows(&self, user: &AuthenticatedUser, ability: &str, args: &[String]) -> bool {
        self.inspect(user, ability, args).is_allowed()
    }

    /// Check if a user is denied from performing an ability.
    pub fn denies(&self, user: &AuthenticatedUser, ability: &str, args: &[String]) -> bool {
        !self.allows(user, ability, args)
    }

    /// Inspect a gate check and return the full result.
    pub fn inspect(&self, user: &AuthenticatedUser, ability: &str, args: &[String]) -> GateCheck {
        // Run before hooks
        let before_hooks = self.before_hooks.lock().unwrap();
        for hook in before_hooks.iter() {
            if let Some(result) = hook(user, ability, args) {
                return result;
            }
        }
        drop(before_hooks);

        // Check gates first
        let result = {
            let abilities = self.abilities.lock().unwrap();
            if let Some(callback) = abilities.get(ability) {
                callback(user, args)
            } else {
                // Fall back to policy check
                let policies = self.policies.lock().unwrap();
                // Try to find a policy for the ability
                // Convention: ability names like "update-post" map to resource "post"
                let resource = ability
                    .rsplit_once('-')
                    .map(|(_, rest)| rest)
                    .or_else(|| ability.rsplit_once('_').map(|(_, rest)| rest))
                    .unwrap_or(ability);
                if let Some(policy) = policies.get(resource) {
                    policy
                        .before(user, ability, args)
                        .or_else(|| policy.check(user, ability, args))
                        .unwrap_or_else(|| {
                            GateCheck::Denied(format!(
                                "Policy '{}' did not handle '{}'.",
                                resource, ability
                            ))
                        })
                } else {
                    GateCheck::Denied(format!("No gate or policy registered for '{}'.", ability))
                }
            }
        };

        // Run after hooks
        let after_hooks = self.after_hooks.lock().unwrap();
        let mut final_result = result;
        for hook in after_hooks.iter() {
            if let Some(override_result) = hook(user, ability, args, &final_result) {
                final_result = override_result;
            }
        }

        final_result
    }

    /// Check if a gate or policy is defined for the given ability.
    pub fn has(&self, ability: &str) -> bool {
        let abilities = self.abilities.lock().unwrap();
        if abilities.contains_key(ability) {
            return true;
        }
        let policies = self.policies.lock().unwrap();
        let resource = ability
            .rsplit_once('-')
            .map(|(_, rest)| rest)
            .or_else(|| ability.rsplit_once('_').map(|(_, rest)| rest))
            .unwrap_or(ability);
        policies.contains_key(resource)
    }

    /// Get a list of all registered ability names.
    pub fn abilities(&self) -> Vec<String> {
        let abilities = self.abilities.lock().unwrap();
        abilities.keys().cloned().collect()
    }

    /// Get a list of all registered policy resource names.
    pub fn policy_resources(&self) -> Vec<String> {
        let policies = self.policies.lock().unwrap();
        policies.keys().cloned().collect()
    }
}

/// Trait for policy classes.
///
/// Policies organize authorization logic around a particular model or resource.
///
/// # Example
///
/// ```ignore
/// struct PostPolicy;
///
/// #[async_trait]
/// impl Policy for PostPolicy {
///     fn resource(&self) -> &str { "post" }
///
///     fn check(&self, user: &AuthenticatedUser, ability: &str, args: &[String]) -> Option<GateCheck> {
///         match ability {
///             "create" => Some(GateCheck::Allowed),
///             "update" => {
///                 let owner_id = args.first()?;
///                 if owner_id == &user.user_id {
///                     Some(GateCheck::Allowed)
///                 } else {
///                     Some(GateCheck::Denied("You do not own this post.".to_string()))
///                 }
///             }
///             _ => None,
///         }
///     }
/// }
/// ```
pub trait Policy: Send + Sync + std::fmt::Debug {
    /// The resource name this policy applies to (e.g. "post", "user").
    fn resource(&self) -> &str;

    /// Check the given ability for the user.
    /// Returns `Some(GateCheck)` if this policy handles the ability, `None` otherwise.
    fn check(&self, user: &AuthenticatedUser, ability: &str, args: &[String]) -> Option<GateCheck>;

    /// Before hook scoped to this policy. Called before `check`.
    /// Returns `Some(GateCheck)` to short-circuit, `None` to proceed to `check`.
    fn before(
        &self,
        _user: &AuthenticatedUser,
        _ability: &str,
        _args: &[String],
    ) -> Option<GateCheck> {
        None
    }
}

/// Check if the authenticated user has a given ability using the Gate in
/// request extensions. Returns `Ok(())` if allowed, `Err(GateCheck)` if denied.
///
/// This is the inner check used by middleware; use `require_ability` to create
/// Axum-compatible middleware layers.
pub async fn check_ability(
    ability: &str,
    user: &AuthenticatedUser,
    gate: &Gate,
) -> Result<(), GateCheck> {
    let result = gate.inspect(user, ability, &[]);
    if result.is_allowed() {
        Ok(())
    } else {
        Err(result)
    }
}

/// Create an Axum middleware layer that checks the given ability.
///
/// Requires both `AuthenticatedUser` and `Gate` to be present in request
/// extensions (inserted by upstream middleware).
///
/// # Example
///
/// ```ignore
/// use axum::{Router, middleware};
/// use axum::routing::get;
/// use larastvel_core::auth::require_ability;
///
/// let app = Router::new()
///     .route("/admin", get(admin_handler))
///     .route_layer(middleware::from_fn(require_ability("admin")));
/// ```
pub fn require_ability(
    ability: &'static str,
) -> impl Fn(
    Request,
    Next,
) -> std::pin::Pin<
    Box<dyn std::future::Future<Output = Result<Response, GateCheck>> + Send>,
> + Send
       + Clone {
    move |req: Request, next: Next| {
        let ability = ability;
        Box::pin(async move {
            let user = req
                .extensions()
                .get::<AuthenticatedUser>()
                .cloned()
                .ok_or_else(|| GateCheck::Denied("Not authenticated.".to_string()))?;

            let gate = req.extensions().get::<Gate>().cloned().ok_or_else(|| {
                GateCheck::Denied(
                    "Gate not initialized. Add Gate to application state.".to_string(),
                )
            })?;

            check_ability(ability, &user, &gate).await?;
            Ok(next.run(req).await)
        })
    }
}

/// Convenience function to check authorization using a global Gate instance.
///
/// Requires the Gate to be stored in a global or passed explicitly. This free
/// function provides a shorthand similar to Laravel's `$this->authorize()`.
pub fn authorize(
    gate: &Gate,
    user: &AuthenticatedUser,
    ability: &str,
    args: &[String],
) -> Result<(), GateCheck> {
    let result = gate.inspect(user, ability, args);
    if result.is_allowed() {
        Ok(())
    } else {
        Err(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::Claims;

    fn test_user(id: &str) -> AuthenticatedUser {
        AuthenticatedUser {
            user_id: id.to_string(),
            claims: Claims {
                sub: id.to_string(),
                exp: 9999999999,
                iat: 0,
            },
        }
    }

    #[test]
    fn test_gate_define_and_allows() {
        let gate = Gate::new();
        gate.define("admin", |user, _| {
            if user.user_id == "admin" {
                GateCheck::Allowed
            } else {
                GateCheck::Denied("Not admin.".to_string())
            }
        });

        let admin = test_user("admin");
        let user = test_user("user-1");

        assert!(gate.allows(&admin, "admin", &[]));
        assert!(!gate.allows(&user, "admin", &[]));
    }

    #[test]
    fn test_gate_denies() {
        let gate = Gate::new();
        gate.define("secret", |user, _| {
            if user.user_id == "root" {
                GateCheck::Allowed
            } else {
                GateCheck::Denied("Access denied.".to_string())
            }
        });

        let user = test_user("user-1");
        assert!(gate.denies(&user, "secret", &[]));
    }

    #[test]
    fn test_gate_inspect() {
        let gate = Gate::new();
        gate.define("delete", |_, _| {
            GateCheck::Denied("Cannot delete.".to_string())
        });

        let user = test_user("user");
        let result = gate.inspect(&user, "delete", &[]);
        assert!(result.is_denied());
        assert_eq!(result.message(), Some("Cannot delete."));
    }

    #[test]
    fn test_gate_with_arguments() {
        let gate = Gate::new();
        gate.define("update-post", |user, args| {
            if args.first().map(|s| s.as_str()) == Some(&user.user_id) {
                GateCheck::Allowed
            } else {
                GateCheck::Denied("Not owned.".to_string())
            }
        });

        let owner = test_user("user-1");
        let other = test_user("user-2");

        assert!(gate.allows(&owner, "update-post", &["user-1".to_string()]));
        assert!(gate.denies(&other, "update-post", &["user-1".to_string()]));
    }

    #[test]
    fn test_before_hook() {
        let gate = Gate::new();
        // A before hook that allows all admin actions
        gate.before(|user, _ability, _args| {
            if user.user_id == "admin" {
                Some(GateCheck::Allowed)
            } else {
                None
            }
        });

        // A gate that denies everything
        gate.define("anything", |_, _| GateCheck::Denied("Nope.".to_string()));

        let admin = test_user("admin");
        let user = test_user("user");

        assert!(gate.allows(&admin, "anything", &[]));
        assert!(gate.denies(&user, "anything", &[]));
    }

    #[test]
    fn test_after_hook() {
        let gate = Gate::new();
        // After hook that denies everything unless it's a super ability
        gate.after(|user, ability, _args, result| {
            if ability == "super-admin" && result.is_allowed() {
                Some(GateCheck::Denied(
                    "Super admin not allowed via after hook.".to_string(),
                ))
            } else if user.user_id == "override" {
                Some(GateCheck::Allowed)
            } else {
                None
            }
        });

        gate.define("super-admin", |_, _| GateCheck::Allowed);
        gate.define("normal", |_, _| GateCheck::Denied("No.".to_string()));

        let admin = test_user("super");
        let overrider = test_user("override");

        assert!(gate.denies(&admin, "super-admin", &[]));
        assert!(gate.allows(&overrider, "normal", &[]));
    }

    #[test]
    fn test_has() {
        let gate = Gate::new();
        gate.define("view-dashboard", |_, _| GateCheck::Allowed);
        assert!(gate.has("view-dashboard"));
        assert!(!gate.has("nonexistent"));
    }

    #[test]
    fn test_abilities_list() {
        let gate = Gate::new();
        gate.define("a", |_, _| GateCheck::Allowed);
        gate.define("b", |_, _| GateCheck::Allowed);
        let mut abilities = gate.abilities();
        abilities.sort();
        assert_eq!(abilities, vec!["a", "b"]);
    }

    #[test]
    fn test_policy_check() {
        #[derive(Debug)]
        struct PostPolicy;

        impl Policy for PostPolicy {
            fn resource(&self) -> &str {
                "post"
            }

            fn check(
                &self,
                user: &AuthenticatedUser,
                ability: &str,
                args: &[String],
            ) -> Option<GateCheck> {
                match ability {
                    "create-post" => Some(GateCheck::Allowed),
                    "update-post" => {
                        let owner_id = args.first()?;
                        if owner_id == &user.user_id {
                            Some(GateCheck::Allowed)
                        } else {
                            Some(GateCheck::Denied("Not your post.".to_string()))
                        }
                    }
                    _ => None,
                }
            }
        }

        let gate = Gate::new();
        let owner = test_user("owner");
        let other = test_user("other");

        // Without a policy registered, it should deny
        assert!(gate.denies(&owner, "create-post", &[]));

        // Register policy
        gate.register_policy("post", Arc::new(PostPolicy));

        assert!(gate.allows(&owner, "create-post", &[]));
        assert!(gate.allows(&owner, "update-post", &["owner".to_string()]));
        assert!(gate.denies(&other, "update-post", &["owner".to_string()]));
    }

    #[test]
    fn test_policy_before_hook() {
        #[derive(Debug)]
        struct AdminPolicy;

        impl Policy for AdminPolicy {
            fn resource(&self) -> &str {
                "admin"
            }

            fn before(
                &self,
                user: &AuthenticatedUser,
                _ability: &str,
                _args: &[String],
            ) -> Option<GateCheck> {
                if user.user_id == "super-admin" {
                    Some(GateCheck::Allowed)
                } else {
                    None
                }
            }

            fn check(
                &self,
                _user: &AuthenticatedUser,
                _ability: &str,
                _args: &[String],
            ) -> Option<GateCheck> {
                Some(GateCheck::Denied("Denied by policy.".to_string()))
            }
        }

        let gate = Gate::new();
        gate.register_policy("admin", Arc::new(AdminPolicy));

        let super_admin = test_user("super-admin");
        let normal = test_user("normal");

        assert!(gate.allows(&super_admin, "access-admin", &[]));
        assert!(gate.denies(&normal, "access-admin", &[]));
    }

    #[test]
    fn test_authorize_function() {
        let gate = Gate::new();
        gate.define("view-reports", |user, _| {
            if user.user_id == "admin" {
                GateCheck::Allowed
            } else {
                GateCheck::Denied("No access.".to_string())
            }
        });

        let admin = test_user("admin");
        let user = test_user("user");

        assert!(authorize(&gate, &admin, "view-reports", &[]).is_ok());
        assert!(authorize(&gate, &user, "view-reports", &[]).is_err());
    }

    #[test]
    fn test_gate_check_utility_methods() {
        let allowed = GateCheck::Allowed;
        assert!(allowed.is_allowed());
        assert!(!allowed.is_denied());
        assert!(allowed.message().is_none());

        let denied = GateCheck::Denied("Forbidden".to_string());
        assert!(!denied.is_allowed());
        assert!(denied.is_denied());
        assert_eq!(denied.message(), Some("Forbidden"));
    }

    #[test]
    fn test_multiple_gates() {
        let gate = Gate::new();
        gate.define("read", |_, _| GateCheck::Allowed);
        gate.define("write", |_, _| {
            GateCheck::Denied("Read-only mode.".to_string())
        });

        let user = test_user("user");
        assert!(gate.allows(&user, "read", &[]));
        assert!(gate.denies(&user, "write", &[]));
    }

    #[test]
    fn test_unknown_ability_returns_denied() {
        let gate = Gate::new();
        let user = test_user("user");
        assert!(gate.denies(&user, "unknown-ability", &[]));
        let result = gate.inspect(&user, "unknown-ability", &[]);
        assert!(result
            .message()
            .unwrap()
            .contains("No gate or policy registered"));
    }

    #[test]
    fn test_policy_resources() {
        #[derive(Debug)]
        struct MyPolicy;
        impl Policy for MyPolicy {
            fn resource(&self) -> &str {
                "my-resource"
            }
            fn check(&self, _: &AuthenticatedUser, _: &str, _: &[String]) -> Option<GateCheck> {
                Some(GateCheck::Allowed)
            }
        }

        let gate = Gate::new();
        gate.register_policy("my-resource", Arc::new(MyPolicy));
        let resources = gate.policy_resources();
        assert_eq!(resources, vec!["my-resource"]);
    }

    #[test]
    fn test_gate_default_impl() {
        let gate = Gate::default();
        assert!(gate.abilities().is_empty());
    }
}
