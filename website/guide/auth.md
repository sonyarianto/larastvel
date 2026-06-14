# Authentication

Larastvel provides JWT-based authentication with guards, policies, password reset, and email verification.

## Auth Service

```rust
// Login
let token = auth.attempt(email, password).await?;
// => Some("jwt-token-string")

// Authenticated user
let user = auth.user().await?;

// Logout
auth.logout(token).await?;
```

## AuthenticatedUser Extractor

```rust
async fn dashboard(AuthUser(user): AuthUser<User>) -> Html<String> {
    Html(format!("Welcome, {}!", user.name))
}
```

## Auth Middleware

```rust
router.get("/dashboard", dashboard)
    .middleware("auth");
```

## Gates & Policies

```rust
// Define a gate
Gate::define("update-post", |user: &User, post: &Post| {
    user.id == post.user_id
});

// Authorize in handler
gate.authorize("update-post", &post)?;

// Policy with CRUD methods
struct PostPolicy;
impl Policy<Post> for PostPolicy {
    fn view(user: &User, post: &Post) -> bool { /* ... */ }
    fn create(user: &User) -> bool { /* ... */ }
    fn update(user: &User, post: &Post) -> bool { /* ... */ }
    fn delete(user: &User, post: &Post) -> bool { /* ... */ }
}

// Before/after hooks
Gate::before(|user, ability| {
    if user.is_admin() { Some(true) } else { None }
});
```

## Password Reset

```rust
let broker = PasswordResetBroker::new(&db, &encrypter);
broker.send_reset_link(&user.email).await?;
broker.reset(token, email, new_password).await?;
```

## Email Verification

```rust
let broker = EmailVerificationBroker::new(&db, &encrypter);
broker.send_verification_email(&user).await?;
broker.verify(token).await?;
// Use VerifiedUser extractor for protected routes
```
