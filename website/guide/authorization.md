# Authorization

Larastvel provides Gates and Policies for authorization, inspired by Laravel.

## Gates

Gates are closures that determine if a user is authorized for an action.

```rust
use larastvel_core::auth::{Gate, authorize, AuthenticatedUser, GateCheck};

// Define a gate — the closure receives the user and string args
let gate = Gate::new();
gate.define("update-post", |user, args| {
    if args.first().map(|s| s.as_str()) == Some(&user.user_id) {
        GateCheck::Allowed
    } else {
        GateCheck::Denied("You do not own this post.".to_string())
    }
});

// Check authorization (sync free function)
let user = AuthenticatedUser {
    user_id: "1".to_string(),
    claims: /* ... */,
};
if authorize(&gate, &user, "update-post", &["1".to_string()]).is_ok() {
    // allowed
}
```

## Policies

Policies organize authorization logic around a resource.

### Defining Policies

Use the `#[policy]` macro to generate the `Policy` trait implementation. See the [full reference](/reference/policies) for details.

```rust
use larastvel_core::auth::{AuthenticatedUser, GateCheck};
use larastvel_core::policy;

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
// Register the middleware alias first — the closure applies a Tower layer to the MethodRouter
router.middleware("auth", |r| r.layer(middleware::from_fn(auth_middleware)));

router.group("/admin", |r| {
    r.with_middleware(vec!["auth"]);
    r.get("/dashboard", admin_dashboard);
});
```

## Helper Functions

| Function | Description |
|----------|-------------|
| `authorize(gate, user, ability, args)` | Sync check, returns `Result<(), GateCheck>` |
| `require_ability(ability)` | Middleware-style check (`Result<Response, GateCheck>`) |
| `check_ability(ability, user, gate)` | Async low-level check |
