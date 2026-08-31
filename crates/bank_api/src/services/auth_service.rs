use bank_db::{RefreshTokensRepository, UsersRepository};
use bank_domain::{AccessToken, AuthError, RefreshToken};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct AuthTokens {
    pub access_token: String,
    pub refresh_token: String,
    pub token_type: &'static str,
    pub expires_in: i64,
}

pub struct AuthService {
    users_repo: UsersRepository,
    refresh_tokens_repo: RefreshTokensRepository,
    jwt_secret: Vec<u8>,
    access_token_duration_secs: i64,
}

impl AuthService {
    pub fn new(
        users_repo: UsersRepository,
        refresh_tokens_repo: RefreshTokensRepository,
        jwt_secret: Vec<u8>,
    ) -> Self {
        AuthService {
            users_repo,
            refresh_tokens_repo,
            jwt_secret,
            access_token_duration_secs: 900, // 15 minutes
        }
    }

    pub async fn login(
        &self,
        user_name: String,
        password_raw: String,
        device_info: Option<String>,
    ) -> Result<AuthTokens, AuthError> {
        let user = self
            .users_repo
            .find_by_user_name(user_name)
            .await
            .map_err(|_| AuthError::InvalidCredentials)?;

        if !user.password.verify(&password_raw) {
            return Err(AuthError::InvalidCredentials);
        }

        let access_token = AccessToken::generate(
            user.user_id,
            &self.jwt_secret,
            self.access_token_duration_secs,
        )?;

        let refresh_token = RefreshToken::generate()?;
        let refresh_hash = refresh_token.hash_sha256();

        self.refresh_tokens_repo
            .create_token(user.user_id, refresh_hash, device_info)
            .await?;

        Ok(AuthTokens {
            access_token,
            refresh_token: refresh_token.into_inner(),
            token_type: "Bearer",
            expires_in: self.access_token_duration_secs,
        })
    }

    pub async fn refresh(&self, refresh_token_raw: String) -> Result<AuthTokens, AuthError> {
        let old_token = RefreshToken::from_raw(refresh_token_raw);
        let old_hash = old_token.hash_sha256();

        let new_token = RefreshToken::generate()?;
        let new_hash = new_token.hash_sha256();

        let user_id = self
            .refresh_tokens_repo
            .verify_and_rotate_token(old_hash, new_hash)
            .await?;

        let access_token = AccessToken::generate(
            user_id,
            &self.jwt_secret,
            self.access_token_duration_secs,
        )?;

        Ok(AuthTokens {
            access_token,
            refresh_token: new_token.into_inner(),
            token_type: "Bearer",
            expires_in: self.access_token_duration_secs,
        })
    }

    pub async fn logout(&self, refresh_token_raw: String) -> Result<(), AuthError> {
        let token = RefreshToken::from_raw(refresh_token_raw);
        let hash = token.hash_sha256();
        self.refresh_tokens_repo.revoke_token(hash).await
    }
}
