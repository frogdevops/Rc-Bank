mod handlers;
mod services;
mod state;

use std::sync::Arc;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use bank_db::{create_oracle_pool, AccountsRepository, UsersRepository};
use oracledb::Pool;
use serde_json::json;
use crate::handlers::{create_account, create_user};
use crate::services::{AccountsService, UsersService};
use crate::state::AppState;

async fn health_check() -> impl IntoResponse {
    Json(json!({
        "status": "ok",
        "message": "Server is running!"
    }))
}

fn create_app(pool: Pool) -> Router {
    let pool_arc = Arc::new(pool);

    let users_repo = UsersRepository::new(pool_arc.clone());
    let accounts_repo = AccountsRepository::new(pool_arc);

    let user_service = Arc::new(UsersService::new(users_repo));
    let account_service = Arc::new(AccountsService::new(accounts_repo));

    let state = AppState {
        user_service,
        account_service,
    };

    Router::new()
        .route("/health", get(health_check))
        .route("/users", post(create_user))
        .route("/accounts", post(create_account))
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

    let app = create_app(pool);

    let host = std::env::var("APP_HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
    let port = std::env::var("APP_PORT").unwrap_or_else(|_| "3000".to_string());
    let addr = format!("{host}:{port}");

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("Failed to bind");
    println!("Listening on: {}", addr);

    axum::serve(listener, app).await.expect("Failed to run server");
}
