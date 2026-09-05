/// End-to-end NATS worker tests for deposit and withdraw operations.
///
/// These tests directly invoke start_deposit_worker / start_withdraw_worker
/// in a background task and verify the full pub → worker → Oracle → reply
/// pipeline without going through the HTTP layer.
use std::sync::Arc;
use chrono::Utc;
use futures::StreamExt;
use bank_api::nats::{connect_nats, ensure_stream};
use bank_api::worker::{start_deposit_worker, start_withdraw_worker};
use bank_db::{create_oracle_pool, AccountsRepository, TransactionsRepository, UsersRepository};
use bank_domain::{
    AccountType, DepositCommand, MoneyResult, WithdrawCommand,
};
use bank_api::services::{AccountsService, TransactionsService, UsersService};

async fn setup() -> (
    Arc<TransactionsService>,
    Arc<AccountsService>,
    Arc<UsersService>,
    async_nats::Client,
) {
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
    let transactions_repo = TransactionsRepository::new(pool.clone());

    let users_svc = Arc::new(UsersService::new(users_repo.clone()));
    let accounts_svc = Arc::new(AccountsService::new(users_repo.clone(), accounts_repo));
    let tx_svc = Arc::new(TransactionsService::new(transactions_repo));

    let nats_url = std::env::var("NATS_URL").unwrap_or_else(|_| "127.0.0.1:4222".to_string());
    let nats = connect_nats(&nats_url).await.expect("NATS connect failed");
    ensure_stream(&nats, "BANK_DEPOSITS", vec!["bank.deposits".into()]).await.ok();
    ensure_stream(&nats, "BANK_WITHDRAWALS", vec!["bank.withdrawals".into()]).await.ok();

    (tx_svc, accounts_svc, users_svc, nats)
}

#[tokio::test]
async fn test_nats_worker_deposit_and_withdraw_end_to_end() {
    let (tx_svc, accounts_svc, users_svc, nats) = setup().await;

    // ── Create a fresh user + account ──────────────────────────────────────────
    let suffix = uuid::Uuid::new_v4().to_string()[..8].to_string();
    let username = format!("nats_dw_{}", suffix);
    let email = format!("nats_dw_{}@test.com", suffix);

    let user = users_svc
        .create_user(
            "Nats".to_string(),
            None,
            "Worker".to_string(),
            username.clone(),
            "SuperSecure123!".to_string(),
            Some(email),
        )
        .await
        .expect("create user failed");

    let account = accounts_svc
        .create_account(user.user_id.clone(), AccountType::Checking)
        .await
        .expect("create account failed");

    let account_id = account.account_id.clone();

    // ── Spawn deposit worker ───────────────────────────────────────────────────
    let dep_nats = nats.clone();
    let dep_svc = tx_svc.clone();
    tokio::spawn(async move {
        start_deposit_worker(dep_nats, dep_svc).await.ok();
    });

    // ── Spawn withdraw worker ──────────────────────────────────────────────────
    let wit_nats = nats.clone();
    let wit_svc = tx_svc.clone();
    tokio::spawn(async move {
        start_withdraw_worker(wit_nats, wit_svc).await.ok();
    });

    // Give the workers a moment to subscribe
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    // ── Test 1: Deposit 10_000 cents ($100) ────────────────────────────────────
    let dep_cid = uuid::Uuid::new_v4().to_string();
    let dep_inbox = nats.new_inbox();
    let mut dep_sub = nats.subscribe(dep_inbox.clone()).await.unwrap();

    let dep_cmd = DepositCommand {
        correlation_id: dep_cid.clone(),
        reply_to: Some(dep_inbox),
        user_id: user.user_id.clone(),
        account_id: account_id.clone(),
        amount_cents: 10_000,
        created_at: Utc::now(),
    };

    nats.publish("bank.deposits", serde_json::to_vec(&dep_cmd).unwrap().into())
        .await
        .expect("publish deposit failed");

    let reply = tokio::time::timeout(
        tokio::time::Duration::from_secs(5),
        dep_sub.next(),
    )
    .await
    .expect("deposit timed out")
    .expect("deposit reply channel closed");

    let dep_result: MoneyResult = serde_json::from_slice(&reply.payload).unwrap();
    assert!(dep_result.success, "Deposit should succeed, got: {:?}", dep_result.error_message);
    assert!(dep_result.transaction.is_some());
    let dep_tx = dep_result.transaction.unwrap();
    assert_eq!(dep_tx.amount_cents, 10_000);
    println!("✅ Deposit succeeded — hash: {}", dep_tx.current_hash);

    // ── Test 2: Withdraw 3_000 cents ($30) ─────────────────────────────────────
    let wit_cid = uuid::Uuid::new_v4().to_string();
    let wit_inbox = nats.new_inbox();
    let mut wit_sub = nats.subscribe(wit_inbox.clone()).await.unwrap();

    let wit_cmd = WithdrawCommand {
        correlation_id: wit_cid.clone(),
        reply_to: Some(wit_inbox),
        user_id: user.user_id.clone(),
        account_id: account_id.clone(),
        amount_cents: 3_000,
        created_at: Utc::now(),
    };

    nats.publish("bank.withdrawals", serde_json::to_vec(&wit_cmd).unwrap().into())
        .await
        .expect("publish withdraw failed");

    let reply = tokio::time::timeout(
        tokio::time::Duration::from_secs(5),
        wit_sub.next(),
    )
    .await
    .expect("withdraw timed out")
    .expect("withdraw reply channel closed");

    let wit_result: MoneyResult = serde_json::from_slice(&reply.payload).unwrap();
    assert!(wit_result.success, "Withdraw should succeed, got: {:?}", wit_result.error_message);
    assert!(wit_result.transaction.is_some());
    let wit_tx = wit_result.transaction.unwrap();
    assert_eq!(wit_tx.amount_cents, -3_000); // withdrawals are stored as negatives (debit)
    println!("✅ Withdraw succeeded — hash: {}", wit_tx.current_hash);

    // ── Test 3: Overdraft should be rejected cleanly ────────────────────────────
    let ov_cid = uuid::Uuid::new_v4().to_string();
    let ov_inbox = nats.new_inbox();
    let mut ov_sub = nats.subscribe(ov_inbox.clone()).await.unwrap();

    let ov_cmd = WithdrawCommand {
        correlation_id: ov_cid.clone(),
        reply_to: Some(ov_inbox),
        user_id: user.user_id.clone(),
        account_id: account_id.clone(),
        amount_cents: 999_999_999,
        created_at: Utc::now(),
    };

    nats.publish("bank.withdrawals", serde_json::to_vec(&ov_cmd).unwrap().into())
        .await
        .unwrap();

    let reply = tokio::time::timeout(
        tokio::time::Duration::from_secs(5),
        ov_sub.next(),
    )
    .await
    .expect("overdraft reply timed out")
    .expect("overdraft reply channel closed");

    let ov_result: MoneyResult = serde_json::from_slice(&reply.payload).unwrap();
    assert!(!ov_result.success, "Overdraft should be rejected");
    let err = ov_result.error_message.unwrap();
    assert!(err.contains("InsufficientFunds"), "Expected InsufficientFunds, got: {}", err);
    println!("✅ Overdraft correctly rejected — error: {}", err);
}
