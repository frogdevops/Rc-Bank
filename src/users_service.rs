use axum::extract::State;
use axum::Json;
use chrono::{DateTime, Utc};
use crate::users::{Users, UsersError, CreateUsersRequest, Name, NewUser, Password, Email};
use crate::users_repository::UsersRepository;
use crate::AppState;

use serde::Serialize;

#[derive(Serialize)]
pub struct UsersResponse {
	pub id: i64,
	pub name: String,
	pub created_at: DateTime<Utc>,
}

pub struct UsersService {
	repo: UsersRepository,
}

impl UsersService {
	pub fn new(repo: UsersRepository) -> Self {
		UsersService { repo }
	}

	pub async fn create_user(&self, req: CreateUsersRequest) -> Result<Users, UsersError> {
		let name = Name::new(req.first_name, req.middle_name, req.last_name)?;
		let password = Password::new(req.password)?;
		let email = match req.email {
			Some(email) => Some(Email::new(email)?),
			None => None
		};
		let new_user = NewUser::new(name, password, req.user_name, email);

		self.repo.insert(new_user).await

	}
}

pub async fn create_user(
	State(state): State<AppState>,
	Json(req): Json<CreateUsersRequest>,
) -> Result<Json<UsersResponse>, UsersError> {
	let user = state.user_service.create_user(req).await?;
	Ok(Json(UsersResponse::from(user)))
}