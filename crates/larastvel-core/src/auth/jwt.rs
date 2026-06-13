use std::time::{SystemTime, UNIX_EPOCH};

use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};

const TOKEN_EXPIRY_SECS: u64 = 86400 * 7; // 7 days

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub exp: usize,
    pub iat: usize,
}

pub fn create_token(user_id: &str, secret: &[u8]) -> Result<String, jsonwebtoken::errors::Error> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as usize;

    let claims = Claims {
        sub: user_id.to_string(),
        exp: now + TOKEN_EXPIRY_SECS as usize,
        iat: now,
    };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret),
    )
}

pub fn verify_token(token: &str, secret: &[u8]) -> Result<Claims, jsonwebtoken::errors::Error> {
    let token_data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret),
        &Validation::default(),
    )?;
    Ok(token_data.claims)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_and_verify() {
        let secret = b"my-secret-key-for-testing";
        let token = create_token("user-1", secret).unwrap();
        let claims = verify_token(&token, secret).unwrap();
        assert_eq!(claims.sub, "user-1");
    }

    #[test]
    fn test_verify_with_wrong_secret() {
        let token = create_token("user-1", b"correct-secret").unwrap();
        let result = verify_token(&token, b"wrong-secret");
        assert!(result.is_err());
    }

    #[test]
    fn test_token_has_claims() {
        let secret = b"test-secret";
        let token = create_token("user-42", secret).unwrap();
        let claims = verify_token(&token, secret).unwrap();
        assert_eq!(claims.sub, "user-42");
        assert!(claims.exp > 0);
        assert!(claims.iat > 0);
        assert!(claims.exp > claims.iat);
    }

    #[test]
    fn test_invalid_token_format() {
        let result = verify_token("not-a-valid-jwt", b"secret");
        assert!(result.is_err());
    }
}
