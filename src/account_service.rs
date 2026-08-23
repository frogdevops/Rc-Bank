use axum::extract::State;
use axum::Json;
use chrono::{DateTime, Utc};
use crate::account::{Account, AccountError, CreateAccountRequest, Name, NewAccount, Password};
use crate::account_repository::AccountRepository;
use crate::AppState;

use serde::Serialize;

#[derive(Serialize)]
pub struct AccountResponse {
	pub id: i64,
	pub name: String,
	pub created_at: DateTime<Utc>,
}

pub struct AccountService {
	repo: AccountRepository,
}

impl AccountService {
	pub fn new(repo: AccountRepository) -> Self {
		AccountService { repo }
	}

	pub async fn create_account(&self, req: CreateAccountRequest) -> Result<Account, AccountError> {
		let name = Name::new(req.name)?;
		let password = Password::new(req.password)?;
		let new_account = NewAccount::new(name, password);

		self.repo.insert(new_account).await

	}
}

pub async fn create_account(
	State(state): State<AppState>,
	Json(req): Json<CreateAccountRequest>,
) -> Result<Json<AccountResponse>, AccountError> {
	let account = state.account_service.create_account(req).await?;
	Ok(Json(AccountResponse::from(account)))
}