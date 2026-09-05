mod extractors;
mod handlers;
mod nats;
mod response;
mod services;
mod state;
mod worker;

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
use crate::worker::{start_deposit_worker, start_transfer_worker, start_withdraw_worker};
use crate::nats::{connect_nats, ensure_stream};

async fn health_check() -> impl IntoResponse {
    Json(json!({
        "status": "ok",
        "message": "Server is running!"
    }))
}

fn create_app(pool: Pool, nats_client: async_nats::Client, jwt_secret: Vec<u8>) -> Router {
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

    // Spawn the background NATS worker for asynchronous transaction execution
    let worker_nats = nats_client.clone();
    let worker_tx = transactions_service.clone();
    tokio::spawn(async move {
        if let Err(e) = start_transfer_worker(worker_nats, worker_tx).await {
            eprintln!("❌ Failed to start NATS transfer worker: {:?}", e);
        }
    });

    let worker_nats = nats_client.clone();
    let worker_tx = transactions_service.clone();
    tokio::spawn(async move {
        if let Err(e) = start_deposit_worker(worker_nats, worker_tx).await {
            eprintln!("❌ Failed to start NATS deposit worker: {:?}", e);
        }
    });

    let worker_nats = nats_client.clone();
    let worker_tx = transactions_service.clone();
    tokio::spawn(async move {
        if let Err(e) = start_withdraw_worker(worker_nats, worker_tx).await {
            eprintln!("❌ Failed to start NATS withdraw worker: {:?}", e);
        }
    });

    let state = AppState {
        user_service,
        account_service,
        auth_service,
        transactions_service,
        nats: nats_client,
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

    let nats_url = std::env::var("NATS_URL").unwrap_or_else(|_| "127.0.0.1:4222".to_string());
    let nats_client = match connect_nats(&nats_url).await {
        Ok(c) => {
            println!("Connected to NATS on {}", nats_url);
            c
        }
        Err(e) => {
            eprintln!("Failed to connect to NATS: {:?}", e);
            std::process::exit(1);
        }
    };

    if let Err(e) = ensure_stream(&nats_client, "BANK_TRANSFERS", vec!["bank.transfers".into()]).await {
        eprintln!("Failed to ensure BANK_TRANSFERS stream: {:?}", e);
    }
    if let Err(e) = ensure_stream(&nats_client, "BANK_DEPOSITS", vec!["bank.deposits".into()]).await {
        eprintln!("Failed to ensure BANK_DEPOSITS stream: {:?}", e);
    }
    if let Err(e) = ensure_stream(&nats_client, "BANK_WITHDRAWALS", vec!["bank.withdrawals".into()]).await {
        eprintln!("Failed to ensure BANK_WITHDRAWALS stream: {:?}", e);
    }

    let app = create_app(pool, nats_client, jwt_secret);

    let host = std::env::var("APP_HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
    let port = std::env::var("APP_PORT").unwrap_or_else(|_| "3000".to_string());
    let addr = format!("{host}:{port}");

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("Failed to bind");
    println!("Listening on: {}", addr);

    axum::serve(listener, app).await.expect("Failed to run server");
}
