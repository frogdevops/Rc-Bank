use std::sync::Arc;
use crate::services::{AccountsService, UsersService};

#[derive(Clone)]
pub struct AppState {
    pub user_service: Arc<UsersService>,
    pub account_service: Arc<AccountsService>,
}
