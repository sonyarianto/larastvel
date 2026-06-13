use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    Aes256Gcm, Nonce,
};
use rand::RngCore;

const NONCE_SIZE: usize = 12;
const KEY_SIZE: usize = 32;

#[derive(Debug, thiserror::Error)]
pub enum EncryptError {
    #[error("Encryption failed: {0}")]
    EncryptionFailed(String),
    #[error("Decryption failed: {0}")]
    DecryptionFailed(String),
    #[error("Invalid key: {0}")]
    InvalidKey(String),
}

pub struct Encrypter {
    cipher: Aes256Gcm,
}

impl Encrypter {
    pub fn new(key: &[u8]) -> Result<Self, EncryptError> {
        if key.len() != KEY_SIZE {
            return Err(EncryptError::InvalidKey(format!(
                "key must be {} bytes, got {}",
                KEY_SIZE,
                key.len()
            )));
        }
        let key_arr: &[u8; KEY_SIZE] = key.try_into().map_err(|_| {
            EncryptError::InvalidKey("key conversion failed".to_string())
        })?;
        let cipher = Aes256Gcm::new_from_slice(key_arr).map_err(|e| {
            EncryptError::InvalidKey(e.to_string())
        })?;
        Ok(Self { cipher })
    }

    pub fn encrypt(&self, plaintext: &str) -> Result<String, EncryptError> {
        let mut nonce_bytes = [0u8; NONCE_SIZE];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = self
            .cipher
            .encrypt(nonce, plaintext.as_bytes())
            .map_err(|e| EncryptError::EncryptionFailed(e.to_string()))?;

        let mut payload = Vec::with_capacity(NONCE_SIZE + ciphertext.len());
        payload.extend_from_slice(&nonce_bytes);
        payload.extend_from_slice(&ciphertext);

        Ok(base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            payload,
        ))
    }

    pub fn decrypt(&self, payload_b64: &str) -> Result<String, EncryptError> {
        let payload = base64::Engine::decode(
            &base64::engine::general_purpose::STANDARD,
            payload_b64,
        )
        .map_err(|e| EncryptError::DecryptionFailed(format!("invalid base64: {}", e)))?;

        if payload.len() < NONCE_SIZE {
            return Err(EncryptError::DecryptionFailed(
                "payload too short".to_string(),
            ));
        }

        let (nonce_bytes, ciphertext) = payload.split_at(NONCE_SIZE);
        let nonce = Nonce::from_slice(nonce_bytes);

        let plaintext = self
            .cipher
            .decrypt(nonce, ciphertext)
            .map_err(|e| EncryptError::DecryptionFailed(e.to_string()))?;

        String::from_utf8(plaintext)
            .map_err(|e| EncryptError::DecryptionFailed(format!("invalid utf-8: {}", e)))
    }

    pub fn generate_key() -> [u8; KEY_SIZE] {
        let mut key = [0u8; KEY_SIZE];
        OsRng.fill_bytes(&mut key);
        key
    }
}

pub fn generate_key() -> String {
    let key = Encrypter::generate_key();
    base64::Engine::encode(&base64::engine::general_purpose::STANDARD, key)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_key() -> Vec<u8> {
        vec![
            0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd,
            0xee, 0xff, 0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb,
            0xcc, 0xdd, 0xee, 0xff,
        ]
    }

    #[test]
    fn test_encrypt_decrypt() {
        let encrypter = Encrypter::new(&test_key()).unwrap();
        let ciphertext = encrypter.encrypt("hello world").unwrap();
        let plaintext = encrypter.decrypt(&ciphertext).unwrap();
        assert_eq!(plaintext, "hello world");
    }

    #[test]
    fn test_encrypt_empty_string() {
        let encrypter = Encrypter::new(&test_key()).unwrap();
        let ciphertext = encrypter.encrypt("").unwrap();
        let plaintext = encrypter.decrypt(&ciphertext).unwrap();
        assert_eq!(plaintext, "");
    }

    #[test]
    fn test_encrypt_unicode() {
        let encrypter = Encrypter::new(&test_key()).unwrap();
        let ciphertext = encrypter.encrypt("héllo wörld 🎉").unwrap();
        let plaintext = encrypter.decrypt(&ciphertext).unwrap();
        assert_eq!(plaintext, "héllo wörld 🎉");
    }

    #[test]
    fn test_encrypt_unique_nonces() {
        let encrypter = Encrypter::new(&test_key()).unwrap();
        let a = encrypter.encrypt("same").unwrap();
        let b = encrypter.encrypt("same").unwrap();
        assert_ne!(a, b);
    }

    #[test]
    fn test_encrypt_decrypt_long_string() {
        let encrypter = Encrypter::new(&test_key()).unwrap();
        let long = "a".repeat(10_000);
        let ciphertext = encrypter.encrypt(&long).unwrap();
        let plaintext = encrypter.decrypt(&ciphertext).unwrap();
        assert_eq!(plaintext, long);
    }

    #[test]
    fn test_decrypt_invalid_base64() {
        let encrypter = Encrypter::new(&test_key()).unwrap();
        let result = encrypter.decrypt("!!!invalid-base64!!!");
        assert!(result.is_err());
    }

    #[test]
    fn test_decrypt_truncated_payload() {
        let encrypter = Encrypter::new(&test_key()).unwrap();
        let result = encrypter.decrypt("dGVzdA==");
        assert!(result.is_err());
    }

    #[test]
    fn test_encrypt_wrong_key() {
        let encrypter_a = Encrypter::new(&test_key()).unwrap();

        let mut wrong_key = test_key();
        wrong_key[0] = 0xff;
        let encrypter_b = Encrypter::new(&wrong_key).unwrap();

        let ciphertext = encrypter_a.encrypt("secret").unwrap();
        let result = encrypter_b.decrypt(&ciphertext);
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_key_size() {
        let result = Encrypter::new(&[0u8; 16]);
        assert!(result.is_err());
    }

    #[test]
    fn test_generate_key() {
        let key1 = generate_key();
        let key2 = generate_key();
        assert_ne!(key1, key2);
        let decoded = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, &key1).unwrap();
        assert_eq!(decoded.len(), 32);
    }
}
