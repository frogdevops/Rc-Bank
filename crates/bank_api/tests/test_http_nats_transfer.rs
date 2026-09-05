use std::sync::Arc;
use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use bank_api::extractors::AuthUser;
use bank_api::handlers::{transfer, TransferRequest};
use bank_api::nats::{connect_nats, ensure_stream};
use bank_api::services::{AccountsService, AuthService, TransactionsService, UsersService};
use bank_api::state::AppState;
use bank_db::{
    create_oracle_pool, AccountsRepository, RefreshTokensRepository, TransactionsRepository,
    UsersRepository,
};
use bank_domain::{AccountType, Amount};

fn setup_pool() -> Arc<oracledb::Pool> {
    dotenvy::from_path("../../.env").ok();
    dotenvy::dotenv().ok();

    let host     = std::env::var("HOST").unwrap_or_else(|_| "localhost".into());
    let port     = std::env::var("PORT").unwrap_or_else(|_| "1521".into());
    let svc      = std::env::var("SERVICE_NAME").expect("SERVICE_NAME must be set");
    let user     = std::env::var("ORACLE_USER").expect("ORACLE_USER must be set");
    let password = std::env::var("DB_PASSWORD").expect("DB_PASSWORD must be set");

    let pool = create_oracle_pool(&host, &port, &svc, &user, &password)
        .expect("Failed to create Oracle pool");
    Arc::new(pool)
}

fn unique_username(prefix: &str) -> String {
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .subsec_nanos();
    format!("{}_{}", prefix, ts)
}

fn cleanup_user(pool: &Arc<oracledb::Pool>, user_name: &str) {
    let conn = pool.acquire().expect("cleanup: acquire connection");
    conn.execute_named(
        "DELETE FROM transactions WHERE account_id IN \
         (SELECT account_id FROM accounts WHERE user_id = \
          (SELECT user_id FROM users WHERE user_name = :u))",
        &[("u", &user_name)],
    ).ok();
    conn.execute_named(
        "DELETE FROM accounts WHERE user_id = \
         (SELECT user_id FROM users WHERE user_name = :u)",
        &[("u", &user_name)],
    ).ok();
    conn.execute_named(
        "DELETE FROM refresh_tokens WHERE user_id = \
         (SELECT user_id FROM users WHERE user_name = :u)",
        &[("u", &user_name)],
    ).ok();
    conn.execute_named(
        "DELETE FROM users WHERE user_name = :u",
        &[("u", &user_name)],
    ).ok();
    conn.commit().ok();
}

#[tokio::test]
async fn test_transfer_handler_via_nats_worker() {
    let pool = setup_pool();
    let jwt_secret = b"test_secret_for_http_transfer_test_key_123".to_vec();

    let users_repo = UsersRepository::new(pool.clone());
    let accounts_repo = AccountsRepository::new(pool.clone());
    let refresh_tokens_repo = RefreshTokensRepository::new(pool.clone());
    let tx_repo = TransactionsRepository::new(pool.clone());

    let users_service = Arc::new(UsersService::new(users_repo.clone()));
    let accounts_service = Arc::new(AccountsService::new(users_repo.clone(), accounts_repo));
    let auth_service = Arc::new(AuthService::new(
        users_repo,
        refresh_tokens_repo,
        jwt_secret.clone(),
    ));
    let transactions_service = Arc::new(TransactionsService::new(tx_repo));

    // Connect to NATS
    let nats_client = connect_nats("127.0.0.1:4222")
        .await
        .expect("Failed to connect to NATS");

    ensure_stream(&nats_client, "BANK_TRANSFERS", vec!["bank.transfers".into()])
        .await
        .expect("Failed to ensure stream");

    // Spawn the background NATS worker
    let worker_nats = nats_client.clone();
    let worker_tx = transactions_service.clone();
    tokio::spawn(async move {
        if let Err(e) = bank_api::worker::start_transfer_worker(worker_nats, worker_tx).await {
            eprintln!("Worker error: {:?}", e);
        }
    });

    // Small delay to ensure queue subscriber is listening
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    let state = AppState {
        user_service: users_service.clone(),
        auth_service: auth_service.clone(),
        transactions_service: transactions_service.clone(),
        nats: nats_client,
        jwt_secret: jwt_secret.clone(),
    };

    // Setup sender & recipient in Oracle
    let sender_name = unique_username("handler_nats_snd");
    let recip_name = unique_username("handler_nats_rcp");

    let sender = users_service.create_user(
        "Clara".into(), None, "Oswald".into(), sender_name.clone(), "Str0ngPass!99".into(), None,
    ).await.unwrap();
    let recip = users_service.create_user(
        "Amy".into(), None, "Pond".into(), recip_name.clone(), "Str0ngPass!99".into(), None,
    ).await.unwrap();

    println!("sender_user: {:?}", sender);
    println!("recip_user: {:?}", recip);
    let sender_acc = accounts_service.create_account(sender.user_id, AccountType::Checking).await
        .expect("sender create_account failed");
    let recip_acc = accounts_service.create_account(recip.user_id, AccountType::Savings).await
        .expect("recip create_account failed");

    // Deposit $1,000 into sender's account
    transactions_service.deposit(sender.user_id, sender_acc.account_id, Amount::new(100_000).unwrap())
        .await
        .unwrap();

    // Call the Axum transfer handler directly!
    let (status, Json(api_res)) = transfer(
        State(state.clone()),
        AuthUser { user_id: sender.user_id },
        Json(TransferRequest {
            from_account_id: sender_acc.account_id.value(),
            to_account_number: recip_acc.account_number.to_string(),
            amount_cents: 35_000,
        }),
    )
    .await
    .expect("Transfer handler failed");

    assert_eq!(status, StatusCode::CREATED);
    let transfer_data = api_res.data.expect("Data present");
    assert_eq!(transfer_data.debit_transaction.amount_cents, -35_000);
    assert_eq!(transfer_data.credit_transaction.amount_cents, 35_000);
    assert!(!transfer_data.debit_transaction.current_hash.is_empty());
    assert!(!transfer_data.credit_transaction.current_hash.is_empty());

    // Verify Oracle balances
    let sender_bal = transactions_service.get_balance(sender.user_id, sender_acc.account_id).await.unwrap();
    let recip_bal = transactions_service.get_balance(recip.user_id, recip_acc.account_id).await.unwrap();
    assert_eq!(sender_bal.cents(), 65_000); // $650.00 left
    assert_eq!(recip_bal.cents(), 35_000);  // $350.00 received

    println!(" Axum transfer handler via NATS completed successfully! Debit hash: {}",
        transfer_data.debit_transaction.current_hash
    );

    cleanup_user(&pool, &sender_name);
    cleanup_user(&pool, &recip_name);
}
