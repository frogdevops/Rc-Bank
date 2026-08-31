use std::sync::Arc;
use crate::services::{AccountsService, AuthService, UsersService};

#[derive(Clone)]
pub struct AppState {
    pub user_service: Arc<UsersService>,
    pub account_service: Arc<AccountsService>,
    pub auth_service: Arc<AuthService>,
    pub jwt_secret: Vec<u8>,
}
