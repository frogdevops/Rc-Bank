use axum::{Json, Router};
use axum::routing::*;
use axum::response::IntoResponse;
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
	let app = create_app();
	let host = std::env::var("HOST")
		.unwrap_or_else(|_| "127.0.0.1".to_string());

	let port = std::env::var("PORT")
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