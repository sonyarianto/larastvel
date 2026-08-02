//! Passkey (WebAuthn) authentication — Laravel's first-party passkey support
//! (shipped in Laravel 13 through Fortify and the starter kits).

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use async_trait::async_trait;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use chrono::{DateTime, Utc};
use ring::digest::{digest, SHA256};
use ring::rand::{SecureRandom, SystemRandom};
use ring::signature::ECDSA_P256_SHA256_ASN1;
use serde::Serialize;
use serde_json::{json, Value};
use uuid::Uuid;

/// Default lifetime of a pending WebAuthn challenge (60 seconds).
pub const DEFAULT_CHALLENGE_TTL: std::time::Duration = std::time::Duration::from_secs(60);

const AUTH_DATA_FLAG_UP: u8 = 0x01;
const AUTH_DATA_FLAG_AT: u8 = 0x40;

/// A stored passkey credential (the public key of a WebAuthn key pair).
#[derive(Debug, Clone, Serialize)]
pub struct PasskeyCredential {
    /// Internal, application-level identifier for this record.
    pub id: String,
    /// The user this credential belongs to.
    pub user_id: String,
    /// Human-friendly label (e.g. "MacBook Air").
    pub name: String,
    /// Credential ID chosen by the authenticator.
    pub credential_id: String,
    /// P-256 subject public key (DER SPKI encoding).
    pub public_key: Vec<u8>,
    /// Signature counter reported by the authenticator.
    pub sign_count: u64,
    pub created_at: DateTime<Utc>,
    pub last_used_at: Option<DateTime<Utc>>,
}

/// Account information consumed by
/// [`PasskeyService::generate_registration_options`].
#[derive(Debug, Clone)]
pub struct PasskeyUserAccount {
    pub id: String,
    pub username: String,
    pub display_name: String,
}

/// Persistence contract for passkey credentials. Implement this against the
/// application's database; a default in-memory store is provided for tests.
#[async_trait]
pub trait PasskeyStore: Send + Sync {
    async fn find_by_credential_id(&self, credential_id: &str) -> Option<PasskeyCredential>;
    async fn find_by_user_id(&self, user_id: &str) -> Vec<PasskeyCredential>;
    async fn upsert(&self, credential: PasskeyCredential);
    async fn delete(&self, credential_id: &str);
}

/// Thread-safe in-memory [`PasskeyStore`] used as the default. Data is lost
/// when the process exits — provide a database-backed store in production.
#[derive(Debug, Clone, Default)]
pub struct MemoryPasskeyStore {
    credentials: Arc<Mutex<Vec<PasskeyCredential>>>,
}

impl MemoryPasskeyStore {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl PasskeyStore for MemoryPasskeyStore {
    async fn find_by_credential_id(&self, credential_id: &str) -> Option<PasskeyCredential> {
        self.credentials
            .lock()
            .unwrap()
            .iter()
            .find(|c| c.credential_id == credential_id)
            .cloned()
    }

    async fn find_by_user_id(&self, user_id: &str) -> Vec<PasskeyCredential> {
        self.credentials
            .lock()
            .unwrap()
            .iter()
            .filter(|c| c.user_id == user_id)
            .cloned()
            .collect()
    }

    async fn upsert(&self, mut credential: PasskeyCredential) {
        let mut list = self.credentials.lock().unwrap();
        if let Some(existing) = list
            .iter_mut()
            .find(|c| c.credential_id == credential.credential_id)
        {
            credential.id = existing.id.clone();
            *existing = credential;
        } else {
            list.push(credential);
        }
    }

    async fn delete(&self, credential_id: &str) {
        self.credentials
            .lock()
            .unwrap()
            .retain(|c| c.credential_id != credential_id);
    }
}

#[derive(Debug, thiserror::Error)]
pub enum PasskeyError {
    #[error("Invalid challenge")]
    InvalidChallenge,
    #[error("Challenge expired")]
    ChallengeExpired,
    #[error("Origin mismatch: {0}")]
    OriginMismatch(String),
    #[error("Invalid attestation: {0}")]
    Attestation(String),
    #[error("Invalid assertion: {0}")]
    Assertion(String),
    #[error("Client data error: {0}")]
    ClientData(String),
    #[error("Credential not found")]
    CredentialNotFound,
    #[error("Cryptographic error: {0}")]
    Crypto(String),
}

struct PendingChallenge {
    user_id: Option<String>,
    expires_at: Instant,
}

/// The WebAuthn relying party: issues challenges and verifies credentials.
#[derive(Clone)]
pub struct PasskeyService {
    rp_id: String,
    rp_name: String,
    origin: String,
    store: Arc<dyn PasskeyStore>,
    challenge_ttl: std::time::Duration,
    rng: Arc<SystemRandom>,
    challenges: Arc<Mutex<HashMap<String, PendingChallenge>>>,
}

impl PasskeyService {
    pub fn new(rp_id: &str, rp_name: &str, origin: &str, store: Arc<dyn PasskeyStore>) -> Self {
        Self {
            rp_id: rp_id.to_string(),
            rp_name: rp_name.to_string(),
            origin: origin.to_string(),
            store,
            challenge_ttl: DEFAULT_CHALLENGE_TTL,
            rng: Arc::new(SystemRandom::new()),
            challenges: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Set how long an issued challenge remains valid (defaults to 60s).
    pub fn challenge_ttl(mut self, ttl: std::time::Duration) -> Self {
        self.challenge_ttl = ttl;
        self
    }

    fn issue_challenge(&self, user_id: Option<String>) -> String {
        let mut bytes = [0u8; 32];
        self.rng.fill(&mut bytes).expect("RNG failure");
        let encoded = URL_SAFE_NO_PAD.encode(bytes);
        self.challenges.lock().unwrap().insert(
            encoded.clone(),
            PendingChallenge {
                user_id,
                expires_at: Instant::now() + self.challenge_ttl,
            },
        );
        encoded
    }

    /// Step 1 of registration: produce options for the browser.
    pub async fn generate_registration_options(
        &self,
        user: &PasskeyUserAccount,
    ) -> Result<Value, PasskeyError> {
        let challenge = self.issue_challenge(Some(user.id.clone()));
        let existing = self.store.find_by_user_id(&user.id).await;
        let exclude_credentials: Vec<Value> = existing
            .iter()
            .map(|c| json!({"type": "public-key", "id": c.credential_id}))
            .collect();
        Ok(json!({
            "challenge": challenge,
            "rp": {"id": self.rp_id, "name": self.rp_name},
            "user": {
                "id": URL_SAFE_NO_PAD.encode(user.id.as_bytes()),
                "name": user.username,
                "displayName": user.display_name,
            },
            "pubKeyCredParams": [{"type": "public-key", "alg": -7}],
            "timeout": 300_000,
            "attestation": "none",
            "excludeCredentials": exclude_credentials,
            "authenticatorSelection": {
                "authenticatorAttachment": "platform",
                "residentKey": "preferred",
                "userVerification": "preferred",
            },
            "extensions": {"credProps": true},
        }))
    }

    /// Step 2 of registration: verify the authenticator's attestation.
    pub async fn verify_registration(
        &self,
        user: &PasskeyUserAccount,
        response: &Value,
    ) -> Result<PasskeyCredential, PasskeyError> {
        let (pending, _client_hash) = self.common_client(response, "webauthn.create")?;
        if pending.user_id.as_deref() != Some(&user.id) {
            return Err(PasskeyError::InvalidChallenge);
        }

        let auth_data = attestation_auth_data(response)?;
        let (credential_id, public_key, counter) = parse_attestation_auth_data(&auth_data)?;

        let credential = PasskeyCredential {
            id: Uuid::new_v4().to_string(),
            user_id: user.id.clone(),
            name: String::new(),
            credential_id,
            public_key,
            sign_count: counter,
            created_at: Utc::now(),
            last_used_at: None,
        };
        self.store.upsert(credential.clone()).await;
        Ok(credential)
    }

    /// Begin a login: produce assertion options for the browser. When `user`
    /// is provided, only that user's credentials are allowed.
    pub async fn generate_assertion_options(
        &self,
        user: Option<&PasskeyUserAccount>,
    ) -> Result<Value, PasskeyError> {
        let challenge = self.issue_challenge(user.map(|u| u.id.clone()));
        let allow_credentials = match user {
            Some(user) => {
                let credentials = self.store.find_by_user_id(&user.id).await;
                Some(
                    credentials
                        .iter()
                        .map(|c| json!({"type": "public-key", "id": c.credential_id}))
                        .collect::<Vec<_>>(),
                )
            }
            None => None,
        };
        Ok(json!({
            "challenge": challenge,
            "rpId": self.rp_id,
            "timeout": 300_000,
            "userVerification": "preferred",
            "allowCredentials": allow_credentials,
            "extensions": {},
        }))
    }

    /// Finish a login: verify the assertion signature and counter.
    pub async fn verify_assertion(
        &self,
        response: &Value,
    ) -> Result<PasskeyCredential, PasskeyError> {
        let (pending, client_hash) = self.common_client(response, "webauthn.get")?;

        let credential_id = response["id"]
            .as_str()
            .ok_or_else(|| PasskeyError::Assertion("missing credential id".into()))?;
        let mut credential = self
            .store
            .find_by_credential_id(credential_id)
            .await
            .ok_or(PasskeyError::CredentialNotFound)?;
        if pending.user_id.is_some() && pending.user_id.as_deref() != Some(&credential.user_id) {
            return Err(PasskeyError::Assertion(
                "credential belongs to another user".into(),
            ));
        }

        let authenticator_data = base64_decode(
            response["response"]["authenticatorData"]
                .as_str()
                .ok_or_else(|| PasskeyError::Assertion("missing authenticatorData".into()))?,
        )?;
        if authenticator_data.len() < 37 {
            return Err(PasskeyError::Assertion(
                "authenticatorData too short".into(),
            ));
        }
        let rp_hash = digest(&SHA256, self.rp_id.as_bytes());
        if authenticator_data[..32] != rp_hash.as_ref()[..] {
            return Err(PasskeyError::Assertion("rpIdHash mismatch".into()));
        }
        let flags = authenticator_data[32];
        if flags & AUTH_DATA_FLAG_UP == 0 {
            return Err(PasskeyError::Assertion("user present flag not set".into()));
        }
        let counter = u32::from_be_bytes([
            authenticator_data[33],
            authenticator_data[34],
            authenticator_data[35],
            authenticator_data[36],
        ]) as u64;
        if counter > 0 && counter <= credential.sign_count {
            return Err(PasskeyError::Assertion(
                "signature counter not increasing".into(),
            ));
        }

        let mut signed_data = authenticator_data.clone();
        signed_data.extend_from_slice(&client_hash);
        let signature = base64_decode(
            response["response"]["signature"]
                .as_str()
                .ok_or_else(|| PasskeyError::Assertion("missing signature".into()))?,
        )?;
        if !verify_ecdsa_p256(&credential.public_key, &signed_data, &signature) {
            return Err(PasskeyError::Assertion("invalid signature".into()));
        }

        credential.sign_count = counter;
        credential.last_used_at = Some(Utc::now());
        self.store.upsert(credential.clone()).await;
        Ok(credential)
    }

    /// Parse and verify the client data shared by both ceremonies, returning
    /// the pending challenge and the SHA-256 of the raw clientDataJSON.
    fn common_client(
        &self,
        response: &Value,
        expected_type: &str,
    ) -> Result<(PendingChallenge, Vec<u8>), PasskeyError> {
        let client_data_json = response["response"]["clientDataJSON"]
            .as_str()
            .ok_or_else(|| PasskeyError::ClientData("missing clientDataJSON".into()))?;
        let bytes = base64_decode(client_data_json)?;
        let parsed: Value = serde_json::from_slice(&bytes)
            .map_err(|e| PasskeyError::ClientData(format!("bad JSON: {e}")))?;
        if parsed["type"].as_str() != Some(expected_type) {
            return Err(PasskeyError::ClientData(format!(
                "expected type {expected_type:?}"
            )));
        }
        let challenge = parsed["challenge"]
            .as_str()
            .ok_or_else(|| PasskeyError::ClientData("missing challenge".into()))?;
        let origin = parsed["origin"]
            .as_str()
            .ok_or_else(|| PasskeyError::ClientData("missing origin".into()))?;
        if origin != self.origin {
            return Err(PasskeyError::OriginMismatch(origin.to_string()));
        }

        let mut challenges = self.challenges.lock().unwrap();
        let Some(pending) = challenges.remove(challenge) else {
            return Err(PasskeyError::InvalidChallenge);
        };
        if pending.expires_at < Instant::now() {
            return Err(PasskeyError::ChallengeExpired);
        }
        let client_hash = digest(&SHA256, &bytes).as_ref().to_vec();
        Ok((pending, client_hash))
    }
}

// ============================================================================
// WebAuthn parsing helpers
// ============================================================================

fn base64_decode(input: &str) -> Result<Vec<u8>, PasskeyError> {
    URL_SAFE_NO_PAD
        .decode(input)
        .map_err(|e| PasskeyError::ClientData(format!("invalid base64: {e}")))
}

/// Extract the raw `authData` from a CBOR attestation object.
fn attestation_auth_data(response: &Value) -> Result<Vec<u8>, PasskeyError> {
    let attestation = base64_decode(
        response["response"]["attestationObject"]
            .as_str()
            .ok_or_else(|| PasskeyError::Attestation("missing attestationObject".into()))?,
    )?;
    let cbor: ciborium::Value = ciborium::from_reader(&mut &attestation[..])
        .map_err(|e| PasskeyError::Attestation(format!("bad CBOR: {e}")))?;
    let ciborium::Value::Map(map) = cbor else {
        return Err(PasskeyError::Attestation("expected CBOR map".into()));
    };
    for (key, value) in &map {
        if let (ciborium::Value::Text(k), ciborium::Value::Bytes(bytes)) = (key, value) {
            if k == "authData" {
                return Ok(bytes.clone());
            }
        }
    }
    Err(PasskeyError::Attestation("missing authData".into()))
}

/// Parse attestation authData: rpIdHash(32) | flags(1) | counter(4) |
/// aaguid(16) | credIdLen(2) | credId | attestedCredentialData (COSE key).
/// Returns `(credential_id, public_key_der, counter)`.
fn parse_attestation_auth_data(auth_data: &[u8]) -> Result<(String, Vec<u8>, u64), PasskeyError> {
    if auth_data.len() < 37 + 16 + 2 {
        return Err(PasskeyError::Attestation("authData too short".into()));
    }
    let flags = auth_data[32];
    if flags & AUTH_DATA_FLAG_AT == 0 {
        return Err(PasskeyError::Attestation(
            "attested data flag not set".into(),
        ));
    }
    let counter =
        u32::from_be_bytes([auth_data[33], auth_data[34], auth_data[35], auth_data[36]]) as u64;
    let aaguid_end = 37 + 16;
    let cred_len = u16::from_be_bytes([auth_data[aaguid_end], auth_data[aaguid_end + 1]]) as usize;
    let cred_start = aaguid_end + 2;
    let cred_end = cred_start + cred_len;
    let credential_id = URL_SAFE_NO_PAD.encode(&auth_data[cred_start..cred_end]);
    let cose_bytes = auth_data
        .get(cred_end..)
        .ok_or_else(|| PasskeyError::Attestation("missing COSE key".into()))?;
    let cose: ciborium::Value = ciborium::from_reader(&mut &cose_bytes[..])
        .map_err(|e| PasskeyError::Attestation(format!("bad COSE: {e}")))?;
    let (x, y) = cose_to_p256_key(&cose)?;
    Ok((credential_id, p256_public_key_point(&x, &y), counter))
}

/// Read a COSE EC2 public key (kty=2, crv=1, alg=-7, x, y).
fn cose_to_p256_key(cose: &ciborium::Value) -> Result<([u8; 32], [u8; 32]), PasskeyError> {
    let ciborium::Value::Map(map) = cose else {
        return Err(PasskeyError::Attestation("expected COSE map".into()));
    };
    let mut x: Option<[u8; 32]> = None;
    let mut y: Option<[u8; 32]> = None;
    let mut crv_ok = false;
    let mut alg_ok = false;
    for (key, value) in map {
        let k = match key {
            ciborium::Value::Integer(i) => i128::from(*i),
            _ => continue,
        };
        match value {
            ciborium::Value::Integer(i) if k == 3 => {
                if i128::from(*i) == -7 {
                    alg_ok = true;
                }
            }
            ciborium::Value::Integer(i) if k == 2 => {
                if i128::from(*i) == 1 {
                    crv_ok = true;
                }
            }
            ciborium::Value::Bytes(b) if k == -1 && b.len() == 32 => {
                let mut arr = [0u8; 32];
                arr.copy_from_slice(b);
                x = Some(arr);
            }
            ciborium::Value::Bytes(b) if k == -2 && b.len() == 32 => {
                let mut arr = [0u8; 32];
                arr.copy_from_slice(b);
                y = Some(arr);
            }
            _ => {}
        }
    }
    if !(alg_ok && crv_ok) {
        return Err(PasskeyError::Attestation("unsupported COSE key".into()));
    }
    let (Some(x), Some(y)) = (x, y) else {
        return Err(PasskeyError::Attestation(
            "COSE key missing coordinates".into(),
        ));
    };
    Ok((x, y))
}

/// Serialize a P-256 coordinate pair as the raw uncompressed point
/// (`0x04 || X || Y`) that ring's ECDSA verifier expects.
fn p256_public_key_point(x: &[u8; 32], y: &[u8; 32]) -> Vec<u8> {
    let mut point = Vec::with_capacity(65);
    point.push(0x04);
    point.extend_from_slice(x);
    point.extend_from_slice(y);
    point
}

fn verify_ecdsa_p256(public_key: &[u8], signed: &[u8], signature: &[u8]) -> bool {
    let key = ring::signature::UnparsedPublicKey::new(&ECDSA_P256_SHA256_ASN1, public_key);
    key.verify(signed, signature).is_ok()
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use ring::signature::{EcdsaKeyPair, KeyPair, ECDSA_P256_SHA256_ASN1_SIGNING};

    const RP_ID: &str = "localhost";
    const RP_ORIGIN: &str = "http://localhost";

    fn account(id: &str, name: &str) -> PasskeyUserAccount {
        PasskeyUserAccount {
            id: id.to_string(),
            username: name.to_string(),
            display_name: name.to_string(),
        }
    }

    fn b64url(data: &[u8]) -> String {
        URL_SAFE_NO_PAD.encode(data)
    }

    fn setup() -> PasskeyService {
        let store = Arc::new(MemoryPasskeyStore::new());
        PasskeyService::new(RP_ID, "Larastvel", RP_ORIGIN, store)
    }

    fn cose_key_bytes(x: [u8; 32], y: [u8; 32]) -> Vec<u8> {
        use ciborium::value::Integer;
        let mut key = Vec::new();
        ciborium::into_writer(
            &ciborium::Value::Map(vec![
                (
                    ciborium::Value::Integer(Integer::from(1)),
                    ciborium::Value::Integer(Integer::from(2)),
                ),
                (
                    ciborium::Value::Integer(Integer::from(2)),
                    ciborium::Value::Integer(Integer::from(1)),
                ),
                (
                    ciborium::Value::Integer(Integer::from(3)),
                    ciborium::Value::Integer(Integer::from(-7)),
                ),
                (
                    ciborium::Value::Integer(Integer::from(-1)),
                    ciborium::Value::Bytes(x.to_vec()),
                ),
                (
                    ciborium::Value::Integer(Integer::from(-2)),
                    ciborium::Value::Bytes(y.to_vec()),
                ),
            ]),
            &mut key,
        )
        .unwrap();
        key
    }

    /// A fake WebAuthn authenticator. It holds a real P-256 key pair and
    /// produces attestations and assertions shaped like a real device.
    struct FakeAuthenticator {
        key: EcdsaKeyPair,
        credential: Vec<u8>,
        counter: u32,
    }

    impl FakeAuthenticator {
        fn new(credential_seed: &[u8; 16]) -> Self {
            let rng = SystemRandom::new();
            let doc = EcdsaKeyPair::generate_pkcs8(&ECDSA_P256_SHA256_ASN1_SIGNING, &rng).unwrap();
            let key = EcdsaKeyPair::from_pkcs8(&ECDSA_P256_SHA256_ASN1_SIGNING, doc.as_ref(), &rng)
                .unwrap();
            let mut credential = vec![0u8; 16];
            credential.copy_from_slice(credential_seed);
            Self {
                key,
                credential,
                counter: 1,
            }
        }

        fn credential_id(&self) -> String {
            b64url(&self.credential)
        }

        fn coordinates(&self) -> ([u8; 32], [u8; 32]) {
            let point = self.key.public_key().as_ref();
            assert_eq!(point.len(), 65, "expected uncompressed P-256 point");
            let mut x = [0u8; 32];
            let mut y = [0u8; 32];
            x.copy_from_slice(&point[1..33]);
            y.copy_from_slice(&point[33..65]);
            (x, y)
        }

        fn attestation(&self, challenge: &str) -> Value {
            let (x, y) = self.coordinates();
            let mut auth_data = Vec::new();
            auth_data.extend_from_slice(digest(&SHA256, RP_ID.as_bytes()).as_ref());
            auth_data.push(AUTH_DATA_FLAG_UP | AUTH_DATA_FLAG_AT);
            auth_data.extend_from_slice(&self.counter.to_be_bytes());
            auth_data.extend_from_slice(&[0u8; 16]);
            auth_data.extend_from_slice(&(self.credential.len() as u16).to_be_bytes());
            auth_data.extend_from_slice(&self.credential);
            auth_data.extend_from_slice(&cose_key_bytes(x, y));

            let client_data = json!({
                "type": "webauthn.create",
                "challenge": challenge,
                "origin": RP_ORIGIN,
            });
            let client_ser = serde_json::to_vec(&client_data).unwrap();

            let mut attestation_obj = Vec::new();
            ciborium::into_writer(
                &ciborium::Value::Map(vec![
                    (
                        ciborium::Value::Text("fmt".into()),
                        ciborium::Value::Text("none".into()),
                    ),
                    (
                        ciborium::Value::Text("attStmt".into()),
                        ciborium::Value::Map(vec![]),
                    ),
                    (
                        ciborium::Value::Text("authData".into()),
                        ciborium::Value::Bytes(auth_data),
                    ),
                ]),
                &mut attestation_obj,
            )
            .unwrap();

            json!({
                "id": self.credential_id(),
                "rawId": self.credential_id(),
                "type": "public-key",
                "response": {
                    "clientDataJSON": b64url(&client_ser),
                    "attestationObject": b64url(&attestation_obj),
                },
                "clientExtensionResults": {},
            })
        }

        fn assertion(&mut self, challenge: &str, credential: &PasskeyCredential) -> Value {
            self.counter += 1;
            let mut auth_data = Vec::new();
            auth_data.extend_from_slice(digest(&SHA256, RP_ID.as_bytes()).as_ref());
            auth_data.push(AUTH_DATA_FLAG_UP);
            auth_data.extend_from_slice(&self.counter.to_be_bytes());

            let client_data = json!({
                "type": "webauthn.get",
                "challenge": challenge,
                "origin": RP_ORIGIN,
            });
            let client_ser = serde_json::to_vec(&client_data).unwrap();
            let client_hash = digest(&SHA256, &client_ser);
            let mut signed = auth_data.clone();
            signed.extend_from_slice(client_hash.as_ref());
            let signature = self.key.sign(&SystemRandom::new(), &signed).unwrap();

            json!({
                "id": credential.credential_id,
                "rawId": credential.credential_id,
                "type": "public-key",
                "response": {
                    "clientDataJSON": b64url(&client_ser),
                    "authenticatorData": b64url(&auth_data),
                    "signature": b64url(signature.as_ref()),
                },
                "clientExtensionResults": {},
            })
        }
    }

    fn base64_decode_str(s: &str) -> Vec<u8> {
        super::base64_decode(s).unwrap()
    }

    #[tokio::test]
    async fn registration_roundtrip() {
        let service = setup();
        let user = account("user-1", "taylor");
        let options = service.generate_registration_options(&user).await.unwrap();
        let challenge = options["challenge"].as_str().unwrap().to_string();

        let auth = FakeAuthenticator::new(&[7u8; 16]);
        let response = auth.attestation(&challenge);

        let credential = service.verify_registration(&user, &response).await.unwrap();
        assert_eq!(credential.user_id, "user-1");
        assert_eq!(credential.credential_id, auth.credential_id());
        assert!(service
            .store
            .find_by_credential_id(&auth.credential_id())
            .await
            .is_some());
    }

    #[tokio::test]
    async fn registration_wrong_user_rejected() {
        let service = setup();
        let user = account("user-1", "taylor");
        let options = service.generate_registration_options(&user).await.unwrap();
        let challenge = options["challenge"].as_str().unwrap().to_string();

        let auth = FakeAuthenticator::new(&[8u8; 16]);
        let response = auth.attestation(&challenge);

        let other = account("user-2", "wrong");
        assert!(service
            .verify_registration(&other, &response)
            .await
            .is_err());
    }

    #[tokio::test]
    async fn registration_origin_mismatch_rejected() {
        let service = setup();
        let user = account("user-1", "taylor");
        let options = service.generate_registration_options(&user).await.unwrap();
        let challenge = options["challenge"].as_str().unwrap().to_string();

        let auth = FakeAuthenticator::new(&[9u8; 16]);
        let mut response = auth.attestation(&challenge);
        let evil = json!({
            "type": "webauthn.create",
            "challenge": challenge,
            "origin": "http://evil.example",
        });
        response["response"]["clientDataJSON"] =
            serde_json::Value::String(b64url(&serde_json::to_vec(&evil).unwrap()));

        assert!(matches!(
            service.verify_registration(&user, &response).await,
            Err(PasskeyError::OriginMismatch(_))
        ));
    }

    #[tokio::test]
    async fn login_roundtrip() {
        let service = setup();
        let user = account("user-1", "alice");

        let reg_options = service.generate_registration_options(&user).await.unwrap();
        let reg_challenge = reg_options["challenge"].as_str().unwrap().to_string();
        let mut auth = FakeAuthenticator::new(&[10u8; 16]);
        let credential = service
            .verify_registration(&user, &auth.attestation(&reg_challenge))
            .await
            .unwrap();
        assert_eq!(credential.sign_count, 1);

        let login_options = service
            .generate_assertion_options(Some(&user))
            .await
            .unwrap();
        let assert_challenge = login_options["challenge"].as_str().unwrap().to_string();
        let assertion = auth.assertion(&assert_challenge, &credential);

        let verified = service.verify_assertion(&assertion).await.unwrap();
        assert_eq!(verified.user_id, "user-1");
        assert_eq!(verified.sign_count, 2);
        assert!(verified.last_used_at.is_some());
    }

    #[tokio::test]
    async fn login_tampered_signature_rejected() {
        let service = setup();
        let user = account("user-1", "alice");
        let reg_options = service.generate_registration_options(&user).await.unwrap();
        let reg_challenge = reg_options["challenge"].as_str().unwrap().to_string();
        let mut auth = FakeAuthenticator::new(&[11u8; 16]);
        let credential = service
            .verify_registration(&user, &auth.attestation(&reg_challenge))
            .await
            .unwrap();

        let login_options = service
            .generate_assertion_options(Some(&user))
            .await
            .unwrap();
        let assert_challenge = login_options["challenge"].as_str().unwrap().to_string();
        let mut assertion = auth.assertion(&assert_challenge, &credential);
        let sig = base64_decode_str(assertion["response"]["signature"].as_str().unwrap());
        let mut flipped = sig;
        flipped[0] ^= 0xff;
        assertion["response"]["signature"] = serde_json::Value::String(b64url(&flipped));

        assert!(matches!(
            service.verify_assertion(&assertion).await,
            Err(PasskeyError::Assertion(_))
        ));
    }

    #[tokio::test]
    async fn challenge_single_use() {
        let service = setup();
        let user = account("user-1", "alice");
        let options = service.generate_registration_options(&user).await.unwrap();
        let challenge = options["challenge"].as_str().unwrap().to_string();

        let auth = FakeAuthenticator::new(&[12u8; 16]);
        let response = auth.attestation(&challenge);
        assert!(service.verify_registration(&user, &response).await.is_ok());
        // Replaying the same challenge must fail.
        assert!(service.verify_registration(&user, &response).await.is_err());
    }
}
