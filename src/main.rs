mod account;
mod account_repository;
mod account_service;

use std::sync::{Arc, Mutex};
use axum::*;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use oracledb::{Config, Connection};
use serde_json::json;
use crate::account_service::AccountService;

#[derive(Clone)]
pub struct AppState {
	pub account_service: Arc<AccountService>,
}
async fn health_check() -> impl IntoResponse {
	Json(json!({
		"status": "ok",
		"message": "Server is running!"
	}))
}

fn create_app(conn: Connection) -> Router {
	let repo = account_repository::AccountRepository::new(Arc::new(Mutex::new(conn)));
	let account_service = Arc::new(account_service::AccountService::new(repo));
	let state = AppState { account_service };

	Router::new()
		.route("/health", get(health_check))
		.route("/create", post(account_service::create_account)).with_state(state)


}
#[tokio::main]
async fn main() {

	dotenvy::dotenv().ok();
	let user = std::env::var("ORACLE_USER").expect("user not found");
	let password = std::env::var("DB_PASSWORD").expect("Something went mismatched");
	let oracle_port = std::env::var("PORT").expect("port not found");
	let oracle_host = std::env::var("HOST").expect("host not found");
	let service_name = std::env::var("SERVICE_NAME").expect("service name not found");

	let config = Config::default()
		.set_connect_string(&format!("{}:{}/{}", oracle_host, oracle_port, service_name))
		.expect("OracleDB config error")
		.set_credentials(&user, &password);

	let conn = match oracledb::connect(config) {
		Ok(connection) => {
			println!("Connected to database");
			connection
		},
		Err(e) => {
			eprintln!("{:?}", e);
			std::process::exit(1);
		},
	};

	let app = create_app(conn);
	let host = std::env::var("APP_HOST")
		.unwrap_or_else(|_| "0.0.0.0".to_string());

	let port = std::env::var("APP_PORT")
		.unwrap_or_else(|_| "3000".to_string());

	let addr = format!("{host}:{port}");
	let listener = tokio::net::TcpListener::bind(&addr)
		.await
		.expect("Failed to bind");
	println!("Listening on: {}", addr);

	axum::serve(listener, app)
		.await
		.expect("Failed to run server");
}