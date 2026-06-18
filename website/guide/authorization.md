# Authorization

Larastvel provides Gates and Policies for authorization, inspired by Laravel.

## Gates

Gates are closures that determine if a user is authorized for an action.

```rust
use larastvel_core::auth::{Gate, authorize};

// Define a gate
Gate::define("update-post", |user, post: &Post| {
    user.id == post.user_id
});

// Check authorization
if authorize("update-post", &post).await {
    // allowed
}
```

## Policies

Policies organize authorization logic around a resource.

### Defining Policies

Use the `#[policy]` macro to generate the `Policy` trait implementation. See the [full reference](/reference/policies) for details.

```rust
use larastvel_core::auth::{AuthenticatedUser, GateCheck};

#[policy("post")]
#[derive(Debug)]
struct PostPolicy;

impl PostPolicy {
    fn check_ability(&self, user: &AuthenticatedUser, ability: &str, args: &[String]) -> Option<GateCheck> {
        match ability {
            "view" | "create" | "update" => Some(GateCheck::Allowed),
            "delete" => Some(GateCheck::Denied("Admins only".to_string())),
            _ => None,
        }
    }
}
```

### Registering Policies

```rust
PostPolicy::register(&gate);
```

## Middleware

Protect routes with the authorization middleware:

```rust
router.group("/admin", |r| {
    r.middleware("auth");
    r.get("/dashboard", admin_dashboard);
});
```

## Helper Functions

| Function | Description |
|----------|-------------|
| `authorize(ability, resource)` | Check authorization |
| `require_ability(ability)` | Middleware-style check |
| `check_ability(user, ability, resource)` | Low-level check |
