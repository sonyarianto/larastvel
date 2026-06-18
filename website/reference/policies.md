# Policies

The `#[policy]` attribute macro generates a `Policy` trait implementation for authorization logic.

## Usage

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

## Arguments

| Argument | Type | Required | Description |
|----------|------|----------|-------------|
| `resource` | string literal | yes | Resource name (e.g. `"post"`, `"user"`) |

## Generated Implementation

The macro generates:

```rust
impl Policy for PostPolicy {
    fn resource(&self) -> &'static str {
        "post"
    }

    fn check(&self, user: &AuthenticatedUser, ability: &str, args: &[String]) -> Option<GateCheck> {
        self.check_ability(user, ability, args)
    }
}
```

Plus a `register()` helper:

```rust
impl PostPolicy {
    pub fn register(gate: &Gate) {
        gate.register_policy(Self::resource_static(), Arc::new(Self));
    }

    fn resource_static() -> &'static str {
        "post"
    }
}
```

## User Method

Your struct must define a `check_ability` method (name chosen to avoid collision with `Policy::check`):

```rust
fn check_ability(&self, user: &AuthenticatedUser, ability: &str, args: &[String]) -> Option<GateCheck>
```

## Registration

```rust
PostPolicy::register(&gate);
```

## CLI Generator

```bash
larastvel make:policy PostPolicy
```
