use std::fmt;
use chrono::{DateTime, Duration, Utc};
use jsonwebtoken::errors::ErrorKind;
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use crate::users::UsersID;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Claims {
    pub sub: i64,        // user_id
    pub exp: usize,      // expiration timestamp
    pub iat: usize,      // issued-at timestamp
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthError {
    InvalidToken,
    ExpiredToken,
    InvalidSignature,
    InvalidCredentials,
    InvalidRefreshToken,
    SessionExpired(String),
    GenerationError(String),
}

impl fmt::Display for AuthError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AuthError::InvalidToken => write!(f, "invalid token"),
            AuthError::ExpiredToken => write!(f, "token has expired"),
            AuthError::InvalidSignature => write!(f, "invalid token signature"),
            AuthError::InvalidCredentials => write!(f, "invalid username or password"),
            AuthError::InvalidRefreshToken => write!(f, "invalid or revoked refresh token"),
            AuthError::SessionExpired(msg) => write!(f, "session expired: {}", msg),
            AuthError::GenerationError(msg) => write!(f, "token generation error: {}", msg),
        }
    }
}

impl std::error::Error for AuthError {}

pub struct AccessToken;

impl AccessToken {
    pub fn generate(user_id: UsersID, secret: &[u8], duration_secs: i64) -> Result<String, AuthError> {
        let now = Utc::now().timestamp();
        let exp = (now + duration_secs) as usize;
        let iat = now as usize;

        let claims = Claims {
            sub: user_id.value(),
            exp,
            iat,
        };

        encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(secret),
        )
        .map_err(|e| AuthError::GenerationError(e.to_string()))
    }

    pub fn verify(token_str: &str, secret: &[u8]) -> Result<Claims, AuthError> {
        let mut validation = Validation::default();
        validation.validate_exp = true;
        validation.leeway = 0;

        let token_data = decode::<Claims>(
            token_str,
            &DecodingKey::from_secret(secret),
            &validation,
        )
        .map_err(|err| match err.kind() {
            ErrorKind::ExpiredSignature => AuthError::ExpiredToken,
            ErrorKind::InvalidSignature => AuthError::InvalidSignature,
            _ => AuthError::InvalidToken,
        })?;

        Ok(token_data.claims)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefreshToken(String);

impl RefreshToken {
    pub fn generate() -> Result<Self, AuthError> {
        let token: String = thread_rng()
            .sample_iter(&Alphanumeric)
            .take(64)
            .map(char::from)
            .collect();

        Ok(RefreshToken(token))
    }

    pub fn from_raw(raw: String) -> Self {
        RefreshToken(raw)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_inner(self) -> String {
        self.0
    }

    pub fn hash_sha256(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(self.0.as_bytes());
        let result = hasher.finalize();
        format!("{:x}", result)
    }

    pub fn verify_hash(&self, expected_hash: &str) -> bool {
        self.hash_sha256().eq_ignore_ascii_case(expected_hash)
    }
}

pub struct SessionPolicy;

impl SessionPolicy {
    pub const INACTIVITY_LIMIT_DAYS: i64 = 3;
    pub const HARD_CAP_LIMIT_DAYS: i64 = 30;

    pub fn validate(
        last_used_at: DateTime<Utc>,
        created_at: DateTime<Utc>,
        now: DateTime<Utc>,
    ) -> Result<(), AuthError> {
        if now - last_used_at > Duration::days(Self::INACTIVITY_LIMIT_DAYS) {
            return Err(AuthError::SessionExpired(
                "session expired due to 3 days of inactivity".to_string(),
            ));
        }

        if now - created_at > Duration::days(Self::HARD_CAP_LIMIT_DAYS) {
            return Err(AuthError::SessionExpired(
                "session exceeded 30-day maximum lifetime".to_string(),
            ));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================
    // 1. JWT ACCESS TOKEN TESTS
    // =========================================================

    #[test]
    fn test_jwt_generation_and_valid_decoding() {
        let user_id = UsersID::from_db(42);
        let secret = b"super_secret_bank_jwt_signing_key_32_bytes_long!";
        let duration_secs = 900; // 15 minutes

        let token = AccessToken::generate(user_id, secret, duration_secs)
            .expect("should generate valid JWT");

        let claims = AccessToken::verify(&token, secret)
            .expect("should verify valid JWT");

        assert_eq!(claims.sub, 42);
        assert!(claims.exp > claims.iat);
    }

    #[test]
    fn test_jwt_expired_token_rejected() {
        let user_id = UsersID::from_db(42);
        let secret = b"super_secret_bank_jwt_signing_key_32_bytes_long!";
        let duration_secs = -10; // Already expired

        let token = AccessToken::generate(user_id, secret, duration_secs)
            .expect("should generate token");

        let err = AccessToken::verify(&token, secret).unwrap_err();
        assert_eq!(err, AuthError::ExpiredToken);
    }

    #[test]
    fn test_jwt_tampered_token_rejected() {
        let user_id = UsersID::from_db(42);
        let secret = b"super_secret_bank_jwt_signing_key_32_bytes_long!";
        let token = AccessToken::generate(user_id, secret, 900).unwrap();

        let tampered = format!("{}tampered", token);
        let err = AccessToken::verify(&tampered, secret).unwrap_err();
        assert!(matches!(err, AuthError::InvalidSignature | AuthError::InvalidToken));
    }

    #[test]
    fn test_jwt_wrong_secret_rejected() {
        let user_id = UsersID::from_db(42);
        let secret = b"correct_secret_key_12345678901234567890!";
        let wrong_secret = b"wrong_secret_key_1234567890123456789000!";

        let token = AccessToken::generate(user_id, secret, 900).unwrap();
        let err = AccessToken::verify(&token, wrong_secret).unwrap_err();
        assert_eq!(err, AuthError::InvalidSignature);
    }

    // =========================================================
    // 2. REFRESH TOKEN (64-CHAR + SHA-256) TESTS
    // =========================================================

    #[test]
    fn test_refresh_token_shape_and_length() {
        let token1 = RefreshToken::generate().expect("should generate refresh token");
        let token2 = RefreshToken::generate().expect("should generate refresh token");

        assert_eq!(token1.as_str().len(), 64, "raw refresh token must be exactly 64 characters");
        assert_ne!(token1.as_str(), token2.as_str(), "successive refresh tokens must be unique/random");
        assert!(token1.as_str().chars().all(|c| c.is_ascii_alphanumeric()), "must be alphanumeric");
    }

    #[test]
    fn test_refresh_token_sha256_hash_and_verification() {
        let token = RefreshToken::generate().unwrap();
        let hash = token.hash_sha256();

        assert_eq!(hash.len(), 64, "SHA-256 hex string must be exactly 64 characters for DB storage");
        assert!(token.verify_hash(&hash), "verification against own hash must succeed");
        assert!(!token.verify_hash("0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"), "verification with different hash must fail");
    }

    // =========================================================
    // 3. SESSION POLICY (3-DAY INACTIVITY & 30-DAY HARD CAP) TESTS
    // =========================================================

    #[test]
    fn test_session_policy_active_session_passes() {
        let now = Utc::now();
        let created_at = now - Duration::days(5);
        let last_used_at = now - Duration::hours(12); // Used 12h ago

        assert!(SessionPolicy::validate(last_used_at, created_at, now).is_ok());
    }

    #[test]
    fn test_session_policy_inactivity_timeout_fails() {
        let now = Utc::now();
        let created_at = now - Duration::days(10);
        let last_used_at = now - Duration::days(4); // 4 days > 3 day limit

        let err = SessionPolicy::validate(last_used_at, created_at, now).unwrap_err();
        assert!(matches!(err, AuthError::SessionExpired(_)));
    }

    #[test]
    fn test_session_policy_hard_cap_timeout_fails() {
        let now = Utc::now();
        let created_at = now - Duration::days(31); // 31 days > 30 day limit
        let last_used_at = now - Duration::hours(1); // Active today, but overall session > 30 days

        let err = SessionPolicy::validate(last_used_at, created_at, now).unwrap_err();
        assert!(matches!(err, AuthError::SessionExpired(_)));
    }
}
