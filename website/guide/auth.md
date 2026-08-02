# Authentication

Larastvel provides JWT-based authentication with gates, policies, password reset, and email verification.

## Auth Service

```rust
use larastvel_core::auth::Auth;

// Construct an Auth instance from the application key
let auth = Auth::from_config(&config);

// Or use the built-in default secret (development only)
let auth = Auth::with_default_secret();

// Create a JWT for a user
let token = auth.create_token("user-1")?;

// Verify a token and get its claims
let claims: Claims = auth.verify_token(&token)?;

// Extract the raw token from an Authorization header ("Bearer <token>")
if let Some(token) = Auth::extract_token_from_header(&headers) {
    // ...
}
```

## AuthenticatedUser Extractor

`AuthenticatedUser` is an Axum extractor that reads the `Authorization` header, verifies the token, and exposes `user_id` plus the raw `Claims`:

```rust
use larastvel_core::auth::AuthenticatedUser;

async fn dashboard(user: AuthenticatedUser) -> Html<String> {
    Html(format!("Welcome, {}!", user.user_id))
}
```

## Auth Middleware

Protect routes with the `auth_middleware` from-fn layer:

```rust
use axum::{middleware, routing::get, Router};
use larastvel_core::auth::auth_middleware;

let app = Router::new()
    .route("/dashboard", get(dashboard))
    .route_layer(middleware::from_fn(auth_middleware));
```

## Gates & Policies

```rust
use larastvel_core::auth::{Gate, AuthenticatedUser, GateCheck};

// Define a gate — the closure receives the user and string args
let gate = Gate::new();
gate.define("update-post", |user, args| {
    if args.first().map(|s| s.as_str()) == Some(&user.user_id) {
        GateCheck::Allowed
    } else {
        GateCheck::Denied("You do not own this post.".to_string())
    }
});

let user = AuthenticatedUser {
    user_id: "1".to_string(),
    claims: /* ... */,
};

// Check authorization
gate.allows(&user, "update-post", &["1".to_string()]);
let check: GateCheck = gate.inspect(&user, "update-post", &["1".to_string()]);
```

Policies organize authorization logic around a resource. Implement the `Policy` trait (`resource` + `check`) and register it:

```rust
use larastvel_core::auth::{AuthenticatedUser, GateCheck, Policy};
use std::sync::Arc;

#[derive(Debug)]
struct PostPolicy;

impl Policy for PostPolicy {
    fn resource(&self) -> &str {
        "post"
    }

    fn check(&self, user: &AuthenticatedUser, ability: &str, args: &[String]) -> Option<GateCheck> {
        match ability {
            "create" => Some(GateCheck::Allowed),
            "update" => {
                if args.first().map(|s| s.as_str()) == Some(&user.user_id) {
                    Some(GateCheck::Allowed)
                } else {
                    Some(GateCheck::Denied("You do not own this post.".to_string()))
                }
            }
            _ => None,
        }
    }
}

let gate = Gate::new();
gate.register_policy("post", Arc::new(PostPolicy));
```

Before/after hooks run around every gate and policy check:

```rust
// Runs before all gates — returning Some(GateCheck) short-circuits
gate.before(|user, _ability, _args| {
    if user.user_id == "admin" {
        Some(GateCheck::Allowed)
    } else {
        None
    }
});

// Runs after all gates — can override the result
gate.after(|user, ability, _args, result| {
    // ...
    None
});
```

## Password Reset

```rust
use larastvel_core::auth::{PasswordResetBroker, PasswordResetConfig};
use larastvel_core::mail::LogMailer;
use std::sync::Arc;

let broker = PasswordResetBroker::new(
    db,                               // sea_orm::DatabaseConnection
    PasswordResetConfig::default(),
    Arc::new(LogMailer::new("log")),  // Arc<dyn Mailer>
    "noreply@example.com",            // from address
    "http://localhost:8080",          // app URL
    "My App",                         // app name
);

broker.send_reset_link("user@example.com").await?;

broker
    .reset("user@example.com", &token, "new-password", |email, hashed| {
        // Update the user's password hash in your database.
        Ok(())
    })
    .await?;
```

## Email Verification

```rust
use larastvel_core::auth::{EmailVerificationBroker, EmailVerificationError, VerifiedUser};
use std::sync::Arc;

let broker = EmailVerificationBroker::new(
    secret,                                  // &[u8] — JWT signing secret
    mailer,                                  // Arc<dyn Mailer>
    "noreply@example.com",                   // from address
    "http://localhost:8080",                 // app URL
    "My App",                                // app name
    Arc::new(|user_id: &str| is_verified(user_id)),            // check_verified
    Arc::new(|user_id: &str| -> Result<(), EmailVerificationError> {
        mark_verified(user_id)                               // mark_verified
    }),
    3600,                                    // token expiry in seconds
);

broker.send_verification_email("user-1", "user@example.com").await?;
let (user_id, email) = broker.verify_token(&token)?;

// Use the VerifiedUser extractor for protected routes
async fn dashboard(user: VerifiedUser) -> Json<Value> {
    Json(json!({ "user_id": user.user_id }))
}
```

## Passkeys (WebAuthn)

Laravel 13 ships first-party passkey support; `PasskeyService` provides the
WebAuthn relying-party logic (challenge issuance, attestation & assertion
verification). Persist credentials by implementing `PasskeyStore`:

```rust
use std::sync::Arc;
use larastvel_core::auth::{
    MemoryPasskeyStore, PasskeyService, PasskeyStore, PasskeyUserAccount,
};

let store: Arc<dyn PasskeyStore> = Arc::new(MemoryPasskeyStore::new());
let service = PasskeyService::new("example.com", "My App", "https://example.com", store);

// Registration — step 1: send the options to the browser
let user = PasskeyUserAccount {
    id: "user-1".to_string(),
    username: "taylor".to_string(),
    display_name: "Taylor".to_string(),
};
let options = service.generate_registration_options(&user).await?;
// ...navigator.credentials.create(options)...

// Registration — step 2: verify the attestation the browser returns
let credential = service.verify_registration(&user, &attestation_response).await?;

// Login — request the challenge (optionally scoped to one user)
let assertion_options = service.generate_assertion_options(Some(&user)).await?;
// ...navigator.credentials.get()...
let credential = service.verify_assertion(&assertion).await?;
```

`verify_registration` and `verify_assertion` validate the challenge
(single-use, 60s TTL by default), origin, `clientDataJSON` type, rpIdHash,
user-present flag and the signature counter. In production, back
`PasskeyService` with a database-backed `PasskeyStore` instead of
`MemoryPasskeyStore`.
