use std::sync::Arc;
use crate::services::{AccountsService, AuthService, TransactionsService, UsersService};

#[derive(Clone)]
pub struct AppState {
    pub user_service: Arc<UsersService>,
    pub account_service: Arc<AccountsService>,
    pub auth_service: Arc<AuthService>,
    pub transactions_service: Arc<TransactionsService>,
    pub jwt_secret: Vec<u8>,
}
