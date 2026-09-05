/// End-to-end NATS worker test for account creation operations.
///
/// Directly invokes start_account_worker in a background task and verifies
/// the full pub → worker → Oracle → reply pipeline for account creation.
use std::sync::Arc;
use chrono::Utc;
use futures::StreamExt;
use bank_api::nats::{connect_nats, ensure_stream};
use bank_api::services::{AccountsService, UsersService};
use bank_api::worker::start_account_worker;
use bank_db::{create_oracle_pool, AccountsRepository, UsersRepository};
use bank_domain::{AccountType, CreateAccountCommand, CreateAccountResult, UsersID};

async fn setup() -> (Arc<AccountsService>, Arc<UsersService>, async_nats::Client) {
    dotenvy::dotenv().ok();
    let user = std::env::var("ORACLE_USER").unwrap();
    let password = std::env::var("DB_PASSWORD").unwrap();
    let port = std::env::var("PORT").unwrap();
    let host = std::env::var("HOST").unwrap();
    let service = std::env::var("SERVICE_NAME").unwrap();
    let pool = create_oracle_pool(&host, &port, &service, &user, &password).unwrap();
    let pool = Arc::new(pool);

    let users_repo = UsersRepository::new(pool.clone());
    let accounts_repo = AccountsRepository::new(pool.clone());

    let users_svc = Arc::new(UsersService::new(users_repo.clone()));
    let accounts_svc = Arc::new(AccountsService::new(users_repo.clone(), accounts_repo));

    let nats_url = std::env::var("NATS_URL").unwrap_or_else(|_| "127.0.0.1:4222".to_string());
    let nats = connect_nats(&nats_url).await.expect("NATS connect failed");
    ensure_stream(&nats, "BANK_ACCOUNTS", vec!["bank.accounts.create".into()])
        .await
        .ok();

    (accounts_svc, users_svc, nats)
}

#[tokio::test]
async fn test_nats_worker_create_account_end_to_end() {
    let (accounts_svc, users_svc, nats) = setup().await;

    // ── Create a fresh user ──────────────────────────────────────────────────
    let suffix = uuid::Uuid::new_v4().to_string()[..8].to_string();
    let username = format!("nats_acc_{}", suffix);
    let email = format!("nats_acc_{}@test.com", suffix);

    let user = users_svc
        .create_user(
            "Account".to_string(),
            None,
            "Tester".to_string(),
            username.clone(),
            "SuperSecure123!".to_string(),
            Some(email),
        )
        .await
        .expect("create user failed");

    // ── Spawn account worker ─────────────────────────────────────────────────
    let worker_nats = nats.clone();
    let worker_svc = accounts_svc.clone();
    tokio::spawn(async move {
        start_account_worker(worker_nats, worker_svc).await.ok();
    });

    // Brief pause for consumer registration
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    // ── Test 1: Create a Savings Account ─────────────────────────────────────
    let cid = uuid::Uuid::new_v4().to_string();
    let inbox = nats.new_inbox();
    let mut sub = nats.subscribe(inbox.clone()).await.unwrap();

    let cmd = CreateAccountCommand {
        correlation_id: cid.clone(),
        reply_to: Some(inbox),
        user_id: user.user_id.clone(),
        account_type: AccountType::Savings,
        created_at: Utc::now(),
    };

    nats.publish("bank.accounts.create", serde_json::to_vec(&cmd).unwrap().into())
        .await
        .expect("publish create account failed");

    let reply = tokio::time::timeout(tokio::time::Duration::from_secs(5), sub.next())
        .await
        .expect("create account timed out")
        .expect("reply channel closed");

    let result: CreateAccountResult = serde_json::from_slice(&reply.payload).unwrap();
    assert!(result.success, "Account creation should succeed, got: {:?}", result.error_message);
    assert!(result.account.is_some());

    let acc = result.account.unwrap();
    assert_eq!(acc.user_id, user.user_id);
    assert_eq!(acc.account_type, AccountType::Savings);
    assert_eq!(acc.balance.cents(), 0);
    assert!(!acc.account_number.as_str().is_empty());
    println!("✅ Account created via NATS — ID: {}, Number: {}", acc.account_id.value(), acc.account_number);

    // ── Test 2: Reject non-existent user ─────────────────────────────────────
    let bad_cid = uuid::Uuid::new_v4().to_string();
    let bad_inbox = nats.new_inbox();
    let mut bad_sub = nats.subscribe(bad_inbox.clone()).await.unwrap();

    let bad_cmd = CreateAccountCommand {
        correlation_id: bad_cid.clone(),
        reply_to: Some(bad_inbox),
        user_id: UsersID::from_db(999_999_999),
        account_type: AccountType::Checking,
        created_at: Utc::now(),
    };

    nats.publish("bank.accounts.create", serde_json::to_vec(&bad_cmd).unwrap().into())
        .await
        .expect("publish bad account failed");

    let bad_reply = tokio::time::timeout(tokio::time::Duration::from_secs(5), bad_sub.next())
        .await
        .expect("bad account timed out")
        .expect("bad reply channel closed");

    let bad_result: CreateAccountResult = serde_json::from_slice(&bad_reply.payload).unwrap();
    assert!(!bad_result.success, "Account creation for nonexistent user should fail");
    let err = bad_result.error_message.unwrap();
    assert!(err.contains("NotFound"), "Expected NotFound, got: {}", err);
    println!("✅ Nonexistent user account creation correctly rejected: {}", err);
}
