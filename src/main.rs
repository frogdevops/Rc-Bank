mod account;

use axum::*;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use oracledb::Config;
use serde_json::json;

async fn health_check() -> impl IntoResponse {
	Json(json!({
		"status": "ok",
		"message": "Server is running!"
	}))
}

fn create_app() -> Router {
	Router::new()
		.route("/health", get(health_check))


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

	match oracledb::connect(config) {
		Ok(_conn) => println!("Connected to database"),
		Err(e) => eprintln!("{:?}", e),
	}

	let app = create_app();
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