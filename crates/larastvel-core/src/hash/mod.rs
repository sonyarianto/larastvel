const BCRYPT_COST: u32 = 12;

#[derive(Debug, thiserror::Error)]
pub enum HashError {
    #[error("Hashing failed: {0}")]
    Failed(String),
}

pub fn make(password: &str) -> Result<String, HashError> {
    bcrypt::hash(password, BCRYPT_COST).map_err(|e| HashError::Failed(e.to_string()))
}

pub fn check(password: &str, hash: &str) -> bool {
    bcrypt::verify(password, hash).unwrap_or(false)
}

pub fn needs_rehash(hash: &str) -> bool {
    if !is_hashed(hash) {
        return true;
    }
    let parts: Vec<&str> = hash.split('$').collect();
    if parts.len() >= 4 {
        if let Ok(cost) = parts[2].parse::<u32>() {
            return cost < BCRYPT_COST;
        }
    }
    true
}

pub fn is_hashed(value: &str) -> bool {
    value.starts_with("$2y$") || value.starts_with("$2b$") || value.starts_with("$2a$")
}

pub fn make_with_cost(password: &str, cost: u32) -> Result<String, HashError> {
    bcrypt::hash(password, cost).map_err(|e| HashError::Failed(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_make_and_check() {
        let hash = make("password123").unwrap();
        assert!(check("password123", &hash));
        assert!(!check("wrong", &hash));
    }

    #[test]
    fn test_hash_needs_rehash() {
        let strong = make_with_cost("test", 12).unwrap();
        assert!(!needs_rehash(&strong));

        let weak = make_with_cost("test", 4).unwrap();
        assert!(needs_rehash(&weak));
    }

    #[test]
    fn test_hash_needs_rehash_invalid() {
        assert!(needs_rehash("not-a-hash"));
        assert!(needs_rehash(""));
    }

    #[test]
    fn test_is_hashed() {
        let hash = make("test").unwrap();
        assert!(is_hashed(&hash));
        assert!(!is_hashed("plaintext"));
    }

    #[test]
    fn test_hash_different_passwords() {
        let h1 = make("abc").unwrap();
        let h2 = make("abc").unwrap();
        assert_ne!(h1, h2);
        assert!(check("abc", &h1));
        assert!(check("abc", &h2));
    }

    #[test]
    fn test_hash_make_with_cost() {
        let hash = make_with_cost("test", 8).unwrap();
        assert!(check("test", &hash));
        let parts: Vec<&str> = hash.split('$').collect();
        assert_eq!(parts[2], "08");
    }

    #[test]
    fn test_hash_empty_password() {
        let hash = make("").unwrap();
        assert!(check("", &hash));
    }
}
