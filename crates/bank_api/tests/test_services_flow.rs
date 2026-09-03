// -----------------------------------------------------------------------------
// tests/test_services_flow.rs
//
// End-to-end integration tests for the bank application's SERVICE LAYER.
// Tests the full flow: Domain Validation -> Business Logic -> DB Round Trip ->
// Zero-Allocation Transposition -> returned_row() -> API Result.
//
// Run all tests:
//   cargo test -p bank_api --test test_services_flow -- --test-threads=1
// -----------------------------------------------------------------------------

use std::sync::Arc;
use bank_api::services::{AccountsService, TransactionsService, UsersService};
use bank_db::{
    create_oracle_pool, AccountsRepository, TransactionsRepository, UsersRepository,
};
use bank_domain::{
    AccountType, Amount, TransactionError, TransactionType, UsersID,
};

// ─────────────────────────────────────────────────────────────────────────────
// Test setup & cleanup helpers
// ─────────────────────────────────────────────────────────────────────────────

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

// ─────────────────────────────────────────────────────────────────────────────
// 7000 series: UsersService Tests
// ─────────────────────────────────────────────────────────────────────────────

/// Test 7000: UsersService validates inputs and creates user through DB round trip.
#[tokio::test]
async fn test_7000_service_create_user_success() {
    let pool = setup_pool();
    let users_repo = UsersRepository::new(Arc::clone(&pool));
    let service = UsersService::new(users_repo);

    let username = unique_username("srv_7000");
    let user = service.create_user(
        "Sophia".into(),
        Some("Grace".into()),
        "Taylor".into(),
        username.clone(),
        "Str0ngP@ssw0rd99!".into(),
        Some(format!("{}@bank.test", username)),
    ).await.expect("test_7000: create_user failed");

    assert!(user.user_id.value() > 0);
    assert_eq!(user.name.full_name(), "Sophia Grace Taylor");
    assert_eq!(user.user_name, username);
    assert!(user.email.is_some());

    cleanup_user(&pool, &username);
    println!("✅ test_7000 passed — service created user with id={}", user.user_id.value());
}

/// Test 7001: UsersService rejects weak password before hitting the database.
#[tokio::test]
async fn test_7001_service_create_user_rejects_weak_password() {
    let pool = setup_pool();
    let users_repo = UsersRepository::new(Arc::clone(&pool));
    let service = UsersService::new(users_repo);

    let username = unique_username("srv_7001");
    let result = service.create_user(
        "Bad".into(),
        None,
        "Password".into(),
        username.clone(),
        "weak".into(), // < 12 characters, no symbols/digits/caps
        None,
    ).await;

    assert!(result.is_err(), "service must reject weak passwords");
    println!("✅ test_7001 passed — weak password correctly rejected by service");
}

// ─────────────────────────────────────────────────────────────────────────────
// 8000 series: AccountsService Tests
// ─────────────────────────────────────────────────────────────────────────────

/// Test 8000: AccountsService verifies user existence and opens account.
#[tokio::test]
async fn test_8000_service_create_account_success() {
    let pool = setup_pool();
    let users_repo = UsersRepository::new(Arc::clone(&pool));
    let accounts_repo = AccountsRepository::new(Arc::clone(&pool));
    let users_service = UsersService::new(users_repo.clone());
    let accounts_service = AccountsService::new(users_repo, accounts_repo);

    let username = unique_username("srv_8000");
    let user = users_service.create_user(
        "Liam".into(),
        None,
        "Johnson".into(),
        username.clone(),
        "Val!dP@ssw0rd99".into(),
        None,
    ).await.unwrap();

    let account = accounts_service.create_account(user.user_id, AccountType::Savings).await
        .expect("test_8000: create_account failed");

    assert!(account.account_id.value() > 0);
    assert!(!account.account_number.as_str().is_empty());
    assert_eq!(account.account_type, AccountType::Savings);
    assert_eq!(account.balance.cents(), 0);

    cleanup_user(&pool, &username);
    println!("✅ test_8000 passed — service created account {}", account.account_number.as_str());
}

/// Test 8001: AccountsService rejects creating account for non-existent user.
#[tokio::test]
async fn test_8001_service_create_account_nonexistent_user() {
    let pool = setup_pool();
    let users_repo = UsersRepository::new(Arc::clone(&pool));
    let accounts_repo = AccountsRepository::new(Arc::clone(&pool));
    let accounts_service = AccountsService::new(users_repo, accounts_repo);

    let fake_user_id = UsersID::from_db(999_999_999);
    let result = accounts_service.create_account(fake_user_id, AccountType::Checking).await;

    assert!(result.is_err(), "must reject account creation for non-existent user");
    println!("✅ test_8001 passed — nonexistent user rejected by accounts service");
}

// ─────────────────────────────────────────────────────────────────────────────
// 9000 series: TransactionsService Business Rules & Security
// ─────────────────────────────────────────────────────────────────────────────

/// Test 9000: TransactionsService deposit with authorization check.
#[tokio::test]
async fn test_9000_service_deposit_success() {
    let pool = setup_pool();
    let users_repo = UsersRepository::new(Arc::clone(&pool));
    let accounts_repo = AccountsRepository::new(Arc::clone(&pool));
    let tx_repo = TransactionsRepository::new(Arc::clone(&pool));

    let users_service = UsersService::new(users_repo.clone());
    let accounts_service = AccountsService::new(users_repo, accounts_repo);
    let tx_service = TransactionsService::new(tx_repo);

    let username = unique_username("srv_9000");
    let user = users_service.create_user(
        "Noah".into(),
        None,
        "Williams".into(),
        username.clone(),
        "Deposit!P@ss99Z".into(),
        None,
    ).await.unwrap();

    let account = accounts_service.create_account(user.user_id, AccountType::Checking).await.unwrap();
    let deposit_amount = Amount::new(75_000).unwrap(); // $750.00

    let tx = tx_service.deposit(user.user_id, account.account_id, deposit_amount).await
        .expect("test_9000: service deposit failed");

    assert_eq!(tx.amount_cents, 75_000);
    assert_eq!(tx.transaction_type, TransactionType::Deposit);

    cleanup_user(&pool, &username);
    println!("✅ test_9000 passed — service deposit executed with auth check");
}

/// Test 9001: TransactionsService rejects deposit if another user tries to deposit into someone else's account.
#[tokio::test]
async fn test_9001_service_rejects_unauthorized_deposit() {
    let pool = setup_pool();
    let users_repo = UsersRepository::new(Arc::clone(&pool));
    let accounts_repo = AccountsRepository::new(Arc::clone(&pool));
    let tx_repo = TransactionsRepository::new(Arc::clone(&pool));

    let users_service = UsersService::new(users_repo.clone());
    let accounts_service = AccountsService::new(users_repo, accounts_repo);
    let tx_service = TransactionsService::new(tx_repo);

    let user_a_name = unique_username("srv_9001_a");
    let user_b_name = unique_username("srv_9001_b");

    let user_a = users_service.create_user(
        "UserA".into(), None, "Owner".into(), user_a_name.clone(), "AuthP@ss!99Za".into(), None,
    ).await.unwrap();
    let user_b = users_service.create_user(
        "UserB".into(), None, "Hacker".into(), user_b_name.clone(), "AuthP@ss!99Zb".into(), None,
    ).await.unwrap();

    let account_a = accounts_service.create_account(user_a.user_id, AccountType::Savings).await.unwrap();

    // User B tries to deposit into User A's account!
    let result = tx_service.deposit(user_b.user_id, account_a.account_id, Amount::new(10_000).unwrap()).await;

    assert!(
        matches!(result, Err(TransactionError::UnauthorizedAccountAccess)),
        "service must block unauthorized access to other users' accounts"
    );

    cleanup_user(&pool, &user_a_name);
    cleanup_user(&pool, &user_b_name);
    println!("✅ test_9001 passed — UnauthorizedAccountAccess correctly thrown");
}

/// Test 9002: TransactionsService transfer by AccountNumber end-to-end.
#[tokio::test]
async fn test_9002_service_transfer_by_account_number() {
    let pool = setup_pool();
    let users_repo = UsersRepository::new(Arc::clone(&pool));
    let accounts_repo = AccountsRepository::new(Arc::clone(&pool));
    let tx_repo = TransactionsRepository::new(Arc::clone(&pool));

    let users_service = UsersService::new(users_repo.clone());
    let accounts_service = AccountsService::new(users_repo, accounts_repo);
    let tx_service = TransactionsService::new(tx_repo.clone());

    let sender_name = unique_username("srv_9002_sender");
    let recip_name  = unique_username("srv_9002_recip");

    let sender = users_service.create_user(
        "Sender".into(), None, "Client".into(), sender_name.clone(), "Transfer!P@ss99Z".into(), None,
    ).await.unwrap();
    let recip = users_service.create_user(
        "Recip".into(), None, "Client".into(), recip_name.clone(), "Transfer!P@ss99Y".into(), None,
    ).await.unwrap();

    let sender_acc = accounts_service.create_account(sender.user_id, AccountType::Checking).await.unwrap();
    let recip_acc  = accounts_service.create_account(recip.user_id, AccountType::Savings).await.unwrap();

    // Initial deposit for sender
    tx_service.deposit(sender.user_id, sender_acc.account_id, Amount::new(200_000).unwrap()).await.unwrap();

    // Transfer $800 to recipient using their AccountNumber!
    let (tx_out, tx_in) = tx_service.transfer(
        sender.user_id,
        sender_acc.account_id,
        recip_acc.account_number.clone(),
        Amount::new(80_000).unwrap(),
    ).await.expect("test_9002: transfer failed");

    assert_eq!(tx_out.amount_cents, -80_000);
    assert_eq!(tx_in.amount_cents, 80_000);

    let sender_bal = tx_repo.get_balance(sender_acc.account_id).await.unwrap();
    let recip_bal  = tx_repo.get_balance(recip_acc.account_id).await.unwrap();

    assert_eq!(sender_bal.cents(), 120_000);
    assert_eq!(recip_bal.cents(), 80_000);

    cleanup_user(&pool, &sender_name);
    cleanup_user(&pool, &recip_name);
    println!("✅ test_9002 passed — service transfer by AccountNumber verified");
}

/// Test 9003: TransactionsService blocks transferring to one's own same account.
#[tokio::test]
async fn test_9003_service_rejects_self_transfer() {
    let pool = setup_pool();
    let users_repo = UsersRepository::new(Arc::clone(&pool));
    let accounts_repo = AccountsRepository::new(Arc::clone(&pool));
    let tx_repo = TransactionsRepository::new(Arc::clone(&pool));

    let users_service = UsersService::new(users_repo.clone());
    let accounts_service = AccountsService::new(users_repo, accounts_repo);
    let tx_service = TransactionsService::new(tx_repo);

    let username = unique_username("srv_9003");
    let user = users_service.create_user(
        "Self".into(), None, "Sender".into(), username.clone(), "Self!Transfer99Z".into(), None,
    ).await.unwrap();
    let account = accounts_service.create_account(user.user_id, AccountType::Checking).await.unwrap();

    // Try transferring to same account
    let result = tx_service.transfer(
        user.user_id,
        account.account_id,
        account.account_number.clone(),
        Amount::new(10_000).unwrap(),
    ).await;

    assert!(
        matches!(result, Err(TransactionError::SelfTransferNotAllowed)),
        "service must reject transfers to the same account"
    );

    cleanup_user(&pool, &username);
    println!("✅ test_9003 passed — SelfTransferNotAllowed correctly thrown");
}
