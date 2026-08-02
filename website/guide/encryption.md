# Encryption & Hashing

## Encryption

Larastvel uses AES-256-GCM for symmetric encryption.

```rust
use larastvel_core::encryption::Encrypter;

// Generate a fresh 32-byte key
let key = Encrypter::generate_key();

// Encrypt
let encrypter = Encrypter::new(&key)?;
let ciphertext = encrypter.encrypt("sensitive data")?;

// Decrypt
let plaintext = encrypter.decrypt(&ciphertext)?;
```

The free `generate_key()` helper returns a base64-encoded string (for storing in config). Decode it back to bytes before constructing the `Encrypter`:

```rust
let encoded: String = generate_key();
let bytes = base64::engine::general_purpose::STANDARD.decode(&encoded)?;
let encrypter = Encrypter::new(&bytes)?;
```

## Hashing

Bcrypt hashing for passwords:

```rust
use larastvel_core::hash;

// Hash a password
let hashed = hash::make("user-password")?;

// Verify
let valid = hash::check("user-password", &hashed)?;

// Check if rehashing is needed (single-argument)
if hash::needs_rehash(&hashed) {
    let new_hash = hash::make("user-password")?;
}
```
