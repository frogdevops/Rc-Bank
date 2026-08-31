use std::sync::Arc;
use bank_domain::{AuthError, UsersID};
use oracledb::Pool;

#[derive(Clone)]
pub struct RefreshTokensRepository {
    pool: Arc<Pool>,
}

impl RefreshTokensRepository {
    pub fn new(pool: Arc<Pool>) -> Self {
        RefreshTokensRepository { pool }
    }

    pub async fn create_token(
        &self,
        user_id: UsersID,
        token_hash: String,
        device_info: Option<String>,
    ) -> Result<(), AuthError> {
        let pool = self.pool.clone();

        tokio::task::spawn_blocking(move || {
            let conn = pool
                .acquire()
                .map_err(|e| AuthError::GenerationError(e.to_string()))?;

            let user_id_val = user_id.value();
            conn.execute_named(
                "INSERT INTO refresh_tokens (user_id, token_hash, device_info, is_verified, is_revoked) \
                 VALUES (:user_id, :token_hash, :device_info, 1, 0)",
                &[
                    ("user_id", &user_id_val),
                    ("token_hash", &token_hash),
                    ("device_info", &device_info.as_deref()),
                ],
            )
            .map_err(|e| AuthError::GenerationError(e.to_string()))?;

            conn.commit()
                .map_err(|e| AuthError::GenerationError(e.to_string()))?;

            Ok(())
        })
        .await
        .map_err(|e| AuthError::GenerationError(e.to_string()))?
    }

    pub async fn verify_and_rotate_token(
        &self,
        old_hash: String,
        new_hash: String,
    ) -> Result<UsersID, AuthError> {
        let pool = self.pool.clone();

        tokio::task::spawn_blocking(move || {
            let conn = pool
                .acquire()
                .map_err(|e| AuthError::GenerationError(e.to_string()))?;

            // 1. Check validity: active (is_revoked = 0), verified (is_verified = 1),
            //    active in last 3 days (rolling window), created within last 30 days (hard cap).
            let row = conn
                .query_row_named(
                    "SELECT token_id, user_id FROM refresh_tokens \
                     WHERE token_hash = :token_hash \
                       AND is_revoked = 0 \
                       AND is_verified = 1 \
                       AND last_used_at >= CURRENT_TIMESTAMP - INTERVAL '3' DAY \
                       AND created_at   >= CURRENT_TIMESTAMP - INTERVAL '30' DAY",
                    &[("token_hash", &old_hash)],
                )
                .map_err(|_| AuthError::InvalidRefreshToken)?;

            let token_id: i64 = row
                .get("token_id")
                .map_err(|e| AuthError::GenerationError(e.to_string()))?;
            let user_id: i64 = row
                .get("user_id")
                .map_err(|e| AuthError::GenerationError(e.to_string()))?;

            // 2. Rotate token hash and update last_used_at
            conn.execute_named(
                "UPDATE refresh_tokens SET token_hash = :new_token_hash, last_used_at = CURRENT_TIMESTAMP \
                 WHERE token_id = :token_id",
                &[
                    ("new_token_hash", &new_hash),
                    ("token_id", &token_id),
                ],
            )
            .map_err(|e| AuthError::GenerationError(e.to_string()))?;

            conn.commit()
                .map_err(|e| AuthError::GenerationError(e.to_string()))?;

            Ok(UsersID::from_db(user_id))
        })
        .await
        .map_err(|e| AuthError::GenerationError(e.to_string()))?
    }

    pub async fn revoke_token(&self, token_hash: String) -> Result<(), AuthError> {
        let pool = self.pool.clone();

        tokio::task::spawn_blocking(move || {
            let conn = pool
                .acquire()
                .map_err(|e| AuthError::GenerationError(e.to_string()))?;

            conn.execute_named(
                "UPDATE refresh_tokens SET is_revoked = 1 WHERE token_hash = :token_hash",
                &[("token_hash", &token_hash)],
            )
            .map_err(|e| AuthError::GenerationError(e.to_string()))?;

            conn.commit()
                .map_err(|e| AuthError::GenerationError(e.to_string()))?;

            Ok(())
        })
        .await
        .map_err(|e| AuthError::GenerationError(e.to_string()))?
    }
}
