# Encryption & Hashing

## Encryption

Larastvel uses AES-256-GCM for symmetric encryption.

```rust
use larastvel_core::encryption::{Encrypter, generate_key};

// Generate a key
let key = generate_key();

// Encrypt
let encrypter = Encrypter::new(&key);
let ciphertext = encrypter.encrypt("sensitive data")?;

// Decrypt
let plaintext = encrypter.decrypt(&ciphertext)?;
```

## Hashing

Bcrypt hashing for passwords:

```rust
use larastvel_core::hash;

// Hash a password
let hashed = hash::make("user-password")?;

// Verify
let valid = hash::check("user-password", &hashed)?;

// Check if rehashing is needed
if hash::needs_rehash(&hashed, 12) {
    let new_hash = hash::make("user-password")?;
}
```
