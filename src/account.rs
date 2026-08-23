use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use argon2::password_hash::rand_core::OsRng;
use argon2::password_hash::SaltString;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use crate::account_service::AccountResponse;

pub(crate) struct Name(String);

impl Name {
	pub fn new(raw: String) -> Result<Name, AccountError> {
		if raw.is_empty() {
			return Err(AccountError::InvalidInput(String::from("name cannot be empty")));
		}
		if raw.chars().count() > 100 {
			return Err(AccountError::InvalidInput(String::from("name is too long")));
		}

		if !raw.chars().all(|c| c.is_alphanumeric() || c.is_whitespace() || c == '-' || c == '\'') {
			return Err(AccountError::InvalidInput("name contains invalid characters".into()));
		}
		Ok(Name(raw))
	}
	pub fn as_str(&self) -> &str {
		&self.0
	}
}
#[derive(Debug, Clone)]
pub(crate) struct AccountID(i64);

impl AccountID {
	pub fn from_db(value: i64) -> Self {
		AccountID(value)
	}
	pub fn value(&self) -> i64 {
		self.0
	}
}
pub(crate) struct Password(String);

impl Password {
	pub fn new(raw: String) -> Result<Password, AccountError> {
		Self::validate_strength(&raw)?;

		let pepper = std::env::var("PEPPER")
			.expect("PEPPER env variable must be set");
		let peppered = format!("{}/{}", pepper, raw);

		let salt = SaltString::generate(&mut OsRng);
		let argon2 = Argon2::default();

		let password_hash = argon2
			.hash_password(peppered.as_bytes(), &salt)
			.map_err(|_| AccountError::InvalidInput(String::from("invalid password")))?
			.to_string();

		Ok(Password(password_hash))
	}

	fn validate_strength(raw: &str) -> Result<(), AccountError> {
		if raw.chars().count() < 12 {
			return Err(AccountError::InvalidInput("password must be at least 12 characters".into()));
		}
		if raw.chars().count() > 128 {
			return Err(AccountError::InvalidInput("password too long".into()));
		}
		if !raw.chars().any(|c| c.is_ascii_uppercase()) {
			return Err(AccountError::InvalidInput("password must contain an uppercase letter".into()));
		}
		if !raw.chars().any(|c| c.is_ascii_lowercase()) {
			return Err(AccountError::InvalidInput("password must contain a lowercase letter".into()));
		}
		if !raw.chars().any(|c| c.is_ascii_digit()) {
			return Err(AccountError::InvalidInput("password must contain a digit".into()));
		}
		if !raw.chars().any(|c| "!@#$%^&*()-_=+[]{}|;:,.<>?".contains(c)) {
			return Err(AccountError::InvalidInput("password must contain a special character".into()));
		}
		if !raw.is_ascii() {
			return Err(AccountError::InvalidInput("password must contain only standard ASCII characters".into()));
		}
		Ok(())
	}

	pub fn from_hash(hash: String) -> Self {
		 Password(hash)
	}

	pub fn verify(&self, attempt: &str) -> bool {
		let pepper = std::env::var("PASSWORD_PEPPER").unwrap_or_default();
		let peppered = format!("{attempt}{pepper}");

		let Ok(parsed_hash) = PasswordHash::new(&self.0) else { return false };
		Argon2::default()
			.verify_password(peppered.as_bytes(), &parsed_hash)
			.is_ok()
	}

	pub fn hash_str(&self) -> &str {
		&self.0
	}
}

pub struct Account {
	pub(crate) name: Name,
	pub(crate) account_id: AccountID,
	pub(crate) password: Password,
	pub(crate) created_at: DateTime<Utc>,
	pub(crate) updated_at: DateTime<Utc>,
}


impl From<Account> for AccountResponse {
	fn from(account: Account) -> Self {
		AccountResponse {
			id: account.account_id.value(),
			name: account.name.as_str().to_string(),
			created_at: account.created_at,
		}
	}
}
pub struct NewAccount {
	pub(crate) name: Name,
	pub(crate) password: Password,
}

impl NewAccount {
	pub(crate) fn new(name: Name, password: Password) -> Self {
		NewAccount { name, password }
	}

	fn name(&self) -> &Name {
		&self.name
	}
	fn password(&self) -> &Password {
		&self.password
	}
}

pub enum AccountError {
	NotFound,
	InvalidInput(String),
	DatabaseError(String),
}

impl IntoResponse for AccountError {
	fn into_response(self) -> Response {
		match self {
			AccountError::NotFound => (StatusCode::NOT_FOUND, "account not found").into_response(),
			AccountError::InvalidInput(msg) => (StatusCode::BAD_REQUEST, msg).into_response(),
			AccountError::DatabaseError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg).into_response(),
		}
	}
}
#[derive(Deserialize)]
pub struct CreateAccountRequest {
	pub(crate) name: String,
	pub(crate) password: String,
}
#[cfg(test)]
mod test {

	use super::*;
	#[test]
	fn accepts_international_names() {
		assert!(Name::new("王小明".to_string()).is_ok());
		assert!(Name::new("José García".to_string()).is_ok());
		assert!(Name::new("François".to_string()).is_ok());
		assert!(Name::new("田中太郎".to_string()).is_ok());
	}

	#[test]
	fn rejects_emoji() {
		assert!(Name::new("🍕Pizza".to_string()).is_err());
		assert!(Name::new("John😀".to_string()).is_err());
	}
}

