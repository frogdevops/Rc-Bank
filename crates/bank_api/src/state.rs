use std::sync::Arc;
use crate::services::{AuthService, TransactionsService, UsersService};

#[derive(Clone)]
pub struct AppState {
    pub user_service: Arc<UsersService>,
    pub auth_service: Arc<AuthService>,
    pub transactions_service: Arc<TransactionsService>,
    pub nats: async_nats::Client,
    pub jwt_secret: Vec<u8>,
}
