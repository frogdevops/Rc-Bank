mod extractors;
mod handlers;
mod response;
mod services;
mod state;

use std::sync::Arc;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use bank_db::{
    create_oracle_pool, AccountsRepository, RefreshTokensRepository, TransactionsRepository,
    UsersRepository,
};
use oracledb::Pool;
use serde_json::json;
use crate::handlers::{
    create_account, create_user, deposit, get_balance, get_statement, login, logout, refresh,
    transfer, withdraw,
};
use crate::services::{AccountsService, AuthService, TransactionsService, UsersService};
use crate::state::AppState;

async fn health_check() -> impl IntoResponse {
    Json(json!({
        "status": "ok",
        "message": "Server is running!"
    }))
}

fn create_app(pool: Pool, jwt_secret: Vec<u8>) -> Router {
    let pool_arc = Arc::new(pool);

    let users_repo = UsersRepository::new(pool_arc.clone());
    let accounts_repo = AccountsRepository::new(pool_arc.clone());
    let refresh_tokens_repo = RefreshTokensRepository::new(pool_arc.clone());
    let transactions_repo = TransactionsRepository::new(pool_arc);

    let user_service = Arc::new(UsersService::new(users_repo.clone()));
    let account_service = Arc::new(AccountsService::new(users_repo.clone(), accounts_repo));
    let auth_service = Arc::new(AuthService::new(
        users_repo,
        refresh_tokens_repo,
        jwt_secret.clone(),
    ));
    let transactions_service = Arc::new(TransactionsService::new(transactions_repo));

    let state = AppState {
        user_service,
        account_service,
        auth_service,
        transactions_service,
        jwt_secret,
    };

    Router::new()
        .route("/health", get(health_check))
        .route("/users", post(create_user))
        .route("/auth/login", post(login))
        .route("/auth/refresh", post(refresh))
        .route("/auth/logout", post(logout))
        .route("/accounts", post(create_account))
        .route("/accounts/{id}/deposit", post(deposit))
        .route("/accounts/{id}/withdraw", post(withdraw))
        .route("/accounts/{id}/balance", get(get_balance))
        .route("/accounts/{id}/statement", get(get_statement))
        .route("/transfers", post(transfer))
        .with_state(state)
}

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();

    let user = std::env::var("ORACLE_USER").expect("ORACLE_USER not found");
    let password = std::env::var("DB_PASSWORD").expect("DB_PASSWORD not found");
    let oracle_port = std::env::var("PORT").expect("PORT not found");
    let oracle_host = std::env::var("HOST").expect("HOST not found");
    let service_name = std::env::var("SERVICE_NAME").expect("SERVICE_NAME not found");
    let jwt_secret = std::env::var("JWT_SECRET").unwrap().into_bytes();

    let pool = match create_oracle_pool(&oracle_host, &oracle_port, &service_name, &user, &password) {
        Ok(p) => {
            println!("Database pool created successfully");
            p
        }
        Err(e) => {
            eprintln!("Pool creation error: {:?}", e);
            std::process::exit(1);
        }
    };

    let app = create_app(pool, jwt_secret);

    let host = std::env::var("APP_HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
    let port = std::env::var("APP_PORT").unwrap_or_else(|_| "3000".to_string());
    let addr = format!("{host}:{port}");

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("Failed to bind");
    println!("Listening on: {}", addr);

    axum::serve(listener, app).await.expect("Failed to run server");
}
