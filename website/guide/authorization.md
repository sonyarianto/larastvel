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

Policies organize authorization logic around a resource:

```rust
use larastvel_core::auth::Policy;

struct PostPolicy;

impl Policy for PostPolicy {
    fn before(user: &User, ability: &str) -> Option<bool> {
        if user.is_admin() {
            return Some(true);
        }
        None
    }
}

// Register checks
Gate::register_policy::<Post, _>("post", PostPolicy);
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
