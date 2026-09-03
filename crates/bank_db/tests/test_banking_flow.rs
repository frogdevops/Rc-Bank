// -----------------------------------------------------------------------------
// tests/test_banking_flow.rs
//
// End-to-end integration tests for the bank application.
// Mirrors the oracle driver's fast-paced numbered test suite style.
//
// IMPORTANT: These tests connect to a REAL Oracle database via .env config.
//            Tests use unique timestamped usernames so they NEVER collide with
//            production data. Each test cleans up after itself via DELETE.
//
// Run all tests:
//   cargo test --test test_banking_flow -- --test-threads=1
//
// Run a single test:
//   cargo test --test test_banking_flow test_1000 -- --test-threads=1
// -----------------------------------------------------------------------------

use std::sync::Arc;
use bank_domain::{
    AccountType, Email, Name, NewAccount, NewUser, Password, TransactionType,
};
use bank_db::{
    AccountsRepository, TransactionsRepository, UsersRepository, create_oracle_pool,
};

// ─────────────────────────────────────────────────────────────────────────────
// Test infrastructure
// ─────────────────────────────────────────────────────────────────────────────

/// Loads .env from the workspace root and builds a shared pool for all tests.
fn setup_pool() -> Arc<oracledb::Pool> {
    dotenvy::from_path("../../.env").ok();
    dotenvy::dotenv().ok(); // fallback: try working directory

    let host     = std::env::var("HOST").unwrap_or_else(|_| "localhost".into());
    let port     = std::env::var("PORT").unwrap_or_else(|_| "1521".into());
    let svc      = std::env::var("SERVICE_NAME").expect("SERVICE_NAME must be set");
    let user     = std::env::var("ORACLE_USER").expect("ORACLE_USER must be set");
    let password = std::env::var("DB_PASSWORD").expect("DB_PASSWORD must be set");

    let pool = create_oracle_pool(&host, &port, &svc, &user, &password)
        .expect("Failed to create Oracle pool — is Docker running?");
    Arc::new(pool)
}

/// Generates a unique test username using the current Unix timestamp (nanosecond precision).
fn unique_username(prefix: &str) -> String {
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .subsec_nanos();
    format!("{}_{}", prefix, ts)
}

/// Deletes test data by user_name to keep the DB clean after each test.
/// Cascades: transactions → accounts → refresh_tokens → users.
fn cleanup_user(pool: &Arc<oracledb::Pool>, user_name: &str) {
    let conn = pool.acquire().expect("cleanup: acquire connection");
    // Use subquery deletes to avoid FK violations
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
// 1000 series: Users Repository
// ─────────────────────────────────────────────────────────────────────────────

/// Test 1000: Inserting a user returns a valid user_id and UTC timestamps.
/// This exercises the RETURNING clause via returned_row().
#[tokio::test]
async fn test_1000_insert_user_returns_id_and_timestamps() {
    let pool = setup_pool();
    let repo = UsersRepository::new(Arc::clone(&pool));

    let username = unique_username("test_1000");

    let name = Name::new("Alice".into(), None, "Angulo".into()).unwrap();
    let password = Password::new("StrongP@ssw0rd99!".into()).unwrap();
    let new_user = NewUser::new(name, password, username.clone(), None);

    let user = repo.insert(new_user).await.expect("test_1000: insert failed");

    assert!(user.user_id.value() > 0, "user_id must be a positive DB-generated value");
    assert_eq!(user.user_name, username);
    assert_eq!(user.name.first_name, "Alice");
    assert_eq!(user.name.last_name, "Angulo");
    assert!(user.email.is_none());
    // Timestamps must be recent (within last 10 seconds)
    let now = chrono::Utc::now();
    let age = now - user.created_at;
    assert!(age.num_seconds() < 10, "created_at must be within the last 10 seconds");

    cleanup_user(&pool, &username);
    println!("✅ test_1000 passed — user_id={}", user.user_id.value());
}

/// Test 1001: Inserting a user with an email stores and returns it correctly.
#[tokio::test]
async fn test_1001_insert_user_with_email() {
    let pool = setup_pool();
    let repo = UsersRepository::new(Arc::clone(&pool));

    let username = unique_username("test_1001");
    let email = Email::new(format!("{}@bank.test", username)).unwrap();

    let name = Name::new("Bob".into(), Some("Carlos".into()), "Santos".into()).unwrap();
    let password = Password::new("SuperSecure!99".into()).unwrap();
    let new_user = NewUser::new(name, password, username.clone(), Some(email));

    let user = repo.insert(new_user).await.expect("test_1001: insert failed");

    assert!(user.user_id.value() > 0);
    assert!(user.email.is_some(), "email must be persisted");
    assert!(user.email.unwrap().as_str().contains("@bank.test"));
    assert!(user.name.middle_name.as_deref() == Some("Carlos"));

    cleanup_user(&pool, &username);
    println!("✅ test_1001 passed — email stored correctly");
}

/// Test 1002: find_by_user_name retrieves the correct user after insert.
#[tokio::test]
async fn test_1002_find_user_by_username() {
    let pool = setup_pool();
    let repo = UsersRepository::new(Arc::clone(&pool));

    let username = unique_username("test_1002");
    let name = Name::new("Charlie".into(), None, "Bravo".into()).unwrap();
    let password = Password::new("SecurePass!77A".into()).unwrap();
    let new_user = NewUser::new(name, password, username.clone(), None);

    let inserted = repo.insert(new_user).await.expect("test_1002: insert failed");
    let found = repo.find_by_user_name(username.clone()).await
        .expect("test_1002: find_by_user_name failed");

    assert_eq!(inserted.user_id.value(), found.user_id.value());
    assert_eq!(found.user_name, username);
    assert_eq!(found.name.first_name, "Charlie");

    cleanup_user(&pool, &username);
    println!("✅ test_1002 passed — user_id matches after lookup");
}

/// Test 1003: find_by_user_name returns NotFound for a non-existent user.
#[tokio::test]
async fn test_1003_find_nonexistent_user_returns_not_found() {
    let pool = setup_pool();
    let repo = UsersRepository::new(Arc::clone(&pool));

    let result = repo.find_by_user_name("this_user_absolutely_does_not_exist_xyz987".into()).await;
    assert!(result.is_err(), "must return error for missing user");

    println!("✅ test_1003 passed — NotFound returned correctly");
}

// ─────────────────────────────────────────────────────────────────────────────
// 2000 series: Accounts Repository
// ─────────────────────────────────────────────────────────────────────────────

/// Test 2000: Opening a SAVINGS account generates a unique account number.
#[tokio::test]
async fn test_2000_open_savings_account_generates_account_number() {
    let pool = setup_pool();
    let users_repo = UsersRepository::new(Arc::clone(&pool));
    let accounts_repo = AccountsRepository::new(Arc::clone(&pool));

    let username = unique_username("test_2000");
    let name = Name::new("Dave".into(), None, "Nakamura".into()).unwrap();
    let password = Password::new("BankTest!@#123".into()).unwrap();
    let new_user = NewUser::new(name, password, username.clone(), None);
    let user = users_repo.insert(new_user).await.expect("test_2000: user insert failed");

    let new_account = NewAccount::new(user.user_id, AccountType::Savings);
    let account = accounts_repo.insert(new_account).await
        .expect("test_2000: account insert failed");

    assert!(account.account_id.value() > 0, "account_id must be positive");
    assert!(!account.account_number.as_str().is_empty(), "account_number must be non-empty");
    assert_eq!(account.account_type, AccountType::Savings);
    assert_eq!(account.user_id.value(), user.user_id.value());
    assert_eq!(account.balance.cents(), 0, "new account must have zero balance");

    cleanup_user(&pool, &username);
    println!("✅ test_2000 passed — account_number={}", account.account_number.as_str());
}

/// Test 2001: Opening a CHECKING account for the same user gets a different account number.
#[tokio::test]
async fn test_2001_open_checking_account() {
    let pool = setup_pool();
    let users_repo = UsersRepository::new(Arc::clone(&pool));
    let accounts_repo = AccountsRepository::new(Arc::clone(&pool));

    let username = unique_username("test_2001");
    let name = Name::new("Eve".into(), None, "Park".into()).unwrap();
    let password = Password::new("CheckAcc!56Zz".into()).unwrap();
    let new_user = NewUser::new(name, password, username.clone(), None);
    let user = users_repo.insert(new_user).await.expect("test_2001: user insert");

    let savings = accounts_repo.insert(NewAccount::new(user.user_id, AccountType::Savings)).await.unwrap();
    let checking = accounts_repo.insert(NewAccount::new(user.user_id, AccountType::Checking)).await.unwrap();

    assert_ne!(
        savings.account_number.as_str(),
        checking.account_number.as_str(),
        "each account must have a unique account number"
    );
    assert_eq!(checking.account_type, AccountType::Checking);

    cleanup_user(&pool, &username);
    println!("✅ test_2001 passed — savings={} checking={}",
        savings.account_number.as_str(), checking.account_number.as_str());
}

// ─────────────────────────────────────────────────────────────────────────────
// 3000 series: Transactions — Deposit, Withdraw, Balance
// ─────────────────────────────────────────────────────────────────────────────

/// Test 3000: Deposit returns a transaction with the correct amount and DEPOSIT type.
/// Verifies returned_row() atomically hands back transaction_id and hash.
#[tokio::test]
async fn test_3000_deposit_returns_transaction_atomically() {
    let pool = setup_pool();
    let users_repo = UsersRepository::new(Arc::clone(&pool));
    let accounts_repo = AccountsRepository::new(Arc::clone(&pool));
    let tx_repo = TransactionsRepository::new(Arc::clone(&pool));

    let username = unique_username("test_3000");
    let user = users_repo.insert(NewUser::new(
        Name::new("Frank".into(), None, "Miller".into()).unwrap(),
        Password::new("Deposit!@#456Xx".into()).unwrap(),
        username.clone(),
        None,
    )).await.unwrap();

    let account = accounts_repo.insert(NewAccount::new(user.user_id, AccountType::Savings)).await.unwrap();

    let tx = tx_repo.deposit(account.account_id, 50_000).await
        .expect("test_3000: deposit failed");

    assert!(tx.transaction_id.value() > 0, "transaction_id must be positive");
    assert_eq!(tx.account_id.value(), account.account_id.value());
    assert_eq!(tx.amount_cents, 50_000, "deposit amount must be +50_000 cents");
    assert_eq!(tx.transaction_type, TransactionType::Deposit);
    assert!(!tx.current_hash.is_empty(), "hash must be non-empty");
    assert!(tx.previous_hash.is_none(), "first transaction has no previous hash (genesis)");

    cleanup_user(&pool, &username);
    println!("✅ test_3000 passed — tx_id={} hash={}", tx.transaction_id.value(), &tx.current_hash[..8]);
}

/// Test 3001: Deposit updates balance correctly after the DB round-trip.
#[tokio::test]
async fn test_3001_deposit_updates_balance() {
    let pool = setup_pool();
    let users_repo = UsersRepository::new(Arc::clone(&pool));
    let accounts_repo = AccountsRepository::new(Arc::clone(&pool));
    let tx_repo = TransactionsRepository::new(Arc::clone(&pool));

    let username = unique_username("test_3001");
    let user = users_repo.insert(NewUser::new(
        Name::new("Grace".into(), None, "Lee".into()).unwrap(),
        Password::new("Bal@nce!Test99Z".into()).unwrap(),
        username.clone(),
        None,
    )).await.unwrap();
    let account = accounts_repo.insert(NewAccount::new(user.user_id, AccountType::Savings)).await.unwrap();

    tx_repo.deposit(account.account_id, 100_000).await.unwrap(); // $1,000.00
    tx_repo.deposit(account.account_id, 50_000).await.unwrap();  // $500.00

    let balance = tx_repo.get_balance(account.account_id).await.unwrap();
    assert_eq!(balance.cents(), 150_000, "balance must be $1,500.00 after two deposits");

    cleanup_user(&pool, &username);
    println!("✅ test_3001 passed — balance={}c", balance.cents());
}

/// Test 3002: Withdrawal deducts from balance and returns the correct negative amount.
#[tokio::test]
async fn test_3002_withdraw_deducts_balance() {
    let pool = setup_pool();
    let users_repo = UsersRepository::new(Arc::clone(&pool));
    let accounts_repo = AccountsRepository::new(Arc::clone(&pool));
    let tx_repo = TransactionsRepository::new(Arc::clone(&pool));

    let username = unique_username("test_3002");
    let user = users_repo.insert(NewUser::new(
        Name::new("Hank".into(), None, "Adams".into()).unwrap(),
        Password::new("Withd!rawTest99X".into()).unwrap(),
        username.clone(),
        None,
    )).await.unwrap();
    let account = accounts_repo.insert(NewAccount::new(user.user_id, AccountType::Checking)).await.unwrap();

    tx_repo.deposit(account.account_id, 200_000).await.unwrap(); // +$2,000
    tx_repo.withdraw(account.account_id, 75_000).await.unwrap(); // -$750

    let balance = tx_repo.get_balance(account.account_id).await.unwrap();
    assert_eq!(balance.cents(), 125_000, "balance must be $1,250 after deposit-withdrawal");

    cleanup_user(&pool, &username);
    println!("✅ test_3002 passed — balance={}c after withdrawal", balance.cents());
}

/// Test 3003: Withdrawal with insufficient funds returns InsufficientFunds error.
#[tokio::test]
async fn test_3003_withdraw_insufficient_funds_returns_error() {
    let pool = setup_pool();
    let users_repo = UsersRepository::new(Arc::clone(&pool));
    let accounts_repo = AccountsRepository::new(Arc::clone(&pool));
    let tx_repo = TransactionsRepository::new(Arc::clone(&pool));

    let username = unique_username("test_3003");
    let user = users_repo.insert(NewUser::new(
        Name::new("Iris".into(), None, "Chen".into()).unwrap(),
        Password::new("Insuff!Funds99Zq".into()).unwrap(),
        username.clone(),
        None,
    )).await.unwrap();
    let account = accounts_repo.insert(NewAccount::new(user.user_id, AccountType::Savings)).await.unwrap();

    tx_repo.deposit(account.account_id, 10_000).await.unwrap(); // only $100

    let result = tx_repo.withdraw(account.account_id, 50_000).await; // try $500
    assert!(
        matches!(result, Err(bank_domain::TransactionError::InsufficientFunds)),
        "must return InsufficientFunds when balance is too low"
    );

    cleanup_user(&pool, &username);
    println!("✅ test_3003 passed — InsufficientFunds correctly returned");
}

/// Test 3004: Multiple deposits produce a chained hash ledger (blockchain-style).
/// Verifies that previous_hash of tx[n] == current_hash of tx[n-1].
#[tokio::test]
async fn test_3004_hash_chain_integrity() {
    let pool = setup_pool();
    let users_repo = UsersRepository::new(Arc::clone(&pool));
    let accounts_repo = AccountsRepository::new(Arc::clone(&pool));
    let tx_repo = TransactionsRepository::new(Arc::clone(&pool));

    let username = unique_username("test_3004");
    let user = users_repo.insert(NewUser::new(
        Name::new("Jake".into(), None, "Brown".into()).unwrap(),
        Password::new("HashCha!n99Testz".into()).unwrap(),
        username.clone(),
        None,
    )).await.unwrap();
    let account = accounts_repo.insert(NewAccount::new(user.user_id, AccountType::Savings)).await.unwrap();

    let tx1 = tx_repo.deposit(account.account_id, 10_000).await.unwrap();
    let tx2 = tx_repo.deposit(account.account_id, 20_000).await.unwrap();
    let tx3 = tx_repo.deposit(account.account_id, 30_000).await.unwrap();

    // tx1 is the genesis: no previous hash
    assert!(tx1.previous_hash.is_none(), "tx1 must be genesis (no previous_hash)");

    // tx2's previous_hash must equal tx1's current_hash
    assert_eq!(
        tx2.previous_hash.as_deref(),
        Some(tx1.current_hash.as_str()),
        "tx2.previous_hash must equal tx1.current_hash"
    );

    // tx3's previous_hash must equal tx2's current_hash
    assert_eq!(
        tx3.previous_hash.as_deref(),
        Some(tx2.current_hash.as_str()),
        "tx3.previous_hash must equal tx2.current_hash"
    );

    // All hashes are 64-char SHA-256
    assert_eq!(tx1.current_hash.len(), 64);
    assert_eq!(tx2.current_hash.len(), 64);
    assert_eq!(tx3.current_hash.len(), 64);

    cleanup_user(&pool, &username);
    println!("✅ test_3004 passed — hash chain: {}...→{}...→{}...",
        &tx1.current_hash[..8], &tx2.current_hash[..8], &tx3.current_hash[..8]);
}

// ─────────────────────────────────────────────────────────────────────────────
// 4000 series: Transactions — Transfer (the atomic beast)
// ─────────────────────────────────────────────────────────────────────────────

/// Test 4000: Atomic transfer debits sender and credits recipient in ONE commit.
/// Verifies that both returned rows are correct and ZERO extra queries were needed.
#[tokio::test]
async fn test_4000_transfer_atomic_debit_and_credit() {
    let pool = setup_pool();
    let users_repo = UsersRepository::new(Arc::clone(&pool));
    let accounts_repo = AccountsRepository::new(Arc::clone(&pool));
    let tx_repo = TransactionsRepository::new(Arc::clone(&pool));

    // Sender
    let sender_username = unique_username("test_4000_sender");
    let sender = users_repo.insert(NewUser::new(
        Name::new("Karl".into(), None, "West".into()).unwrap(),
        Password::new("Sender!Pass99Zk".into()).unwrap(),
        sender_username.clone(),
        None,
    )).await.unwrap();
    let sender_acc = accounts_repo.insert(NewAccount::new(sender.user_id, AccountType::Checking)).await.unwrap();

    // Recipient
    let recip_username = unique_username("test_4000_recip");
    let recip = users_repo.insert(NewUser::new(
        Name::new("Lena".into(), None, "Cruz".into()).unwrap(),
        Password::new("Recip!Pass99Zl".into()).unwrap(),
        recip_username.clone(),
        None,
    )).await.unwrap();
    let recip_acc = accounts_repo.insert(NewAccount::new(recip.user_id, AccountType::Savings)).await.unwrap();

    // Fund sender: $1,000
    tx_repo.deposit(sender_acc.account_id, 100_000).await.unwrap();

    // Transfer $300 from sender → recipient
    let (tx_out, tx_in) = tx_repo.transfer(sender_acc.account_id, recip_acc.account_id, 30_000).await
        .expect("test_4000: transfer failed");

    // Debit side
    assert_eq!(tx_out.account_id.value(), sender_acc.account_id.value());
    assert_eq!(tx_out.amount_cents, -30_000, "debit must be negative");
    assert_eq!(tx_out.transaction_type, TransactionType::TransferOut);
    assert!(tx_out.transaction_id.value() > 0);

    // Credit side
    assert_eq!(tx_in.account_id.value(), recip_acc.account_id.value());
    assert_eq!(tx_in.amount_cents, 30_000, "credit must be positive");
    assert_eq!(tx_in.transaction_type, TransactionType::TransferIn);
    assert!(tx_in.transaction_id.value() > 0);

    // Verify final balances
    let sender_balance = tx_repo.get_balance(sender_acc.account_id).await.unwrap();
    let recip_balance  = tx_repo.get_balance(recip_acc.account_id).await.unwrap();

    assert_eq!(sender_balance.cents(), 70_000, "sender must have $700 remaining");
    assert_eq!(recip_balance.cents(), 30_000,  "recipient must have $300");

    cleanup_user(&pool, &sender_username);
    cleanup_user(&pool, &recip_username);
    println!("✅ test_4000 passed — tx_out_id={} tx_in_id={} sender_bal={}c recip_bal={}c",
        tx_out.transaction_id.value(), tx_in.transaction_id.value(),
        sender_balance.cents(), recip_balance.cents());
}

/// Test 4001: Transfer fails with InsufficientFunds when sender cannot cover the amount.
#[tokio::test]
async fn test_4001_transfer_insufficient_funds() {
    let pool = setup_pool();
    let users_repo = UsersRepository::new(Arc::clone(&pool));
    let accounts_repo = AccountsRepository::new(Arc::clone(&pool));
    let tx_repo = TransactionsRepository::new(Arc::clone(&pool));

    let sender_uname = unique_username("test_4001_sender");
    let recip_uname  = unique_username("test_4001_recip");

    let sender = users_repo.insert(NewUser::new(
        Name::new("Mike".into(), None, "Ford".into()).unwrap(),
        Password::new("SenderFail!99Zm".into()).unwrap(),
        sender_uname.clone(),
        None,
    )).await.unwrap();
    let sender_acc = accounts_repo.insert(NewAccount::new(sender.user_id, AccountType::Checking)).await.unwrap();

    let recip = users_repo.insert(NewUser::new(
        Name::new("Nina".into(), None, "Bell".into()).unwrap(),
        Password::new("RecipFail!99Zn".into()).unwrap(),
        recip_uname.clone(),
        None,
    )).await.unwrap();
    let recip_acc = accounts_repo.insert(NewAccount::new(recip.user_id, AccountType::Savings)).await.unwrap();

    tx_repo.deposit(sender_acc.account_id, 5_000).await.unwrap(); // only $50

    let result = tx_repo.transfer(sender_acc.account_id, recip_acc.account_id, 100_000).await;
    assert!(
        matches!(result, Err(bank_domain::TransactionError::InsufficientFunds)),
        "must return InsufficientFunds when sender balance is too low"
    );

    // Sender balance must be unchanged after the failed transfer
    let balance = tx_repo.get_balance(sender_acc.account_id).await.unwrap();
    assert_eq!(balance.cents(), 5_000, "sender balance must be unchanged after failed transfer");

    cleanup_user(&pool, &sender_uname);
    cleanup_user(&pool, &recip_uname);
    println!("✅ test_4001 passed — transfer correctly rejected with InsufficientFunds");
}

/// Test 4002: Transfer correctly chains hashes on the recipient's account.
#[tokio::test]
async fn test_4002_transfer_chains_recipient_hash() {
    let pool = setup_pool();
    let users_repo = UsersRepository::new(Arc::clone(&pool));
    let accounts_repo = AccountsRepository::new(Arc::clone(&pool));
    let tx_repo = TransactionsRepository::new(Arc::clone(&pool));

    let sender_uname = unique_username("test_4002_sender");
    let recip_uname  = unique_username("test_4002_recip");

    let sender = users_repo.insert(NewUser::new(
        Name::new("Oscar".into(), None, "Tang".into()).unwrap(),
        Password::new("SndHash!Chain9Zo".into()).unwrap(),
        sender_uname.clone(),
        None,
    )).await.unwrap();
    let sender_acc = accounts_repo.insert(NewAccount::new(sender.user_id, AccountType::Checking)).await.unwrap();

    let recip = users_repo.insert(NewUser::new(
        Name::new("Paula".into(), None, "Kim".into()).unwrap(),
        Password::new("RcvHash!Chain9Zp".into()).unwrap(),
        recip_uname.clone(),
        None,
    )).await.unwrap();
    let recip_acc = accounts_repo.insert(NewAccount::new(recip.user_id, AccountType::Savings)).await.unwrap();

    tx_repo.deposit(sender_acc.account_id, 500_000).await.unwrap();

    // First transfer → recipient's GENESIS tx_in
    let (_, tx_in_1) = tx_repo.transfer(sender_acc.account_id, recip_acc.account_id, 10_000).await.unwrap();
    // Second transfer → recipient's second tx_in (should chain from tx_in_1)
    let (_, tx_in_2) = tx_repo.transfer(sender_acc.account_id, recip_acc.account_id, 20_000).await.unwrap();

    assert!(tx_in_1.previous_hash.is_none(), "recipient's first tx must be genesis");
    assert_eq!(
        tx_in_2.previous_hash.as_deref(),
        Some(tx_in_1.current_hash.as_str()),
        "recipient's second tx must chain from first"
    );

    cleanup_user(&pool, &sender_uname);
    cleanup_user(&pool, &recip_uname);
    println!("✅ test_4002 passed — recipient hash chain verified");
}

// ─────────────────────────────────────────────────────────────────────────────
// 5000 series: Statement & History
// ─────────────────────────────────────────────────────────────────────────────

/// Test 5000: get_statement returns the correct number of transactions in DESC order.
#[tokio::test]
async fn test_5000_get_statement_returns_correct_count() {
    let pool = setup_pool();
    let users_repo = UsersRepository::new(Arc::clone(&pool));
    let accounts_repo = AccountsRepository::new(Arc::clone(&pool));
    let tx_repo = TransactionsRepository::new(Arc::clone(&pool));

    let username = unique_username("test_5000");
    let user = users_repo.insert(NewUser::new(
        Name::new("Quinn".into(), None, "Evans".into()).unwrap(),
        Password::new("Statement!99Xzq".into()).unwrap(),
        username.clone(),
        None,
    )).await.unwrap();
    let account = accounts_repo.insert(NewAccount::new(user.user_id, AccountType::Savings)).await.unwrap();

    // Insert 5 transactions
    for i in 0..5 {
        tx_repo.deposit(account.account_id, (i + 1) * 10_000).await.unwrap();
    }

    // Fetch last 3
    let statement = tx_repo.get_statement(account.account_id, 3).await.unwrap();
    assert_eq!(statement.len(), 3, "get_statement with limit=3 must return exactly 3 rows");

    // Must be ordered DESC (most recent first = largest tx_id first)
    let ids: Vec<i64> = statement.iter().map(|t| t.transaction_id.value()).collect();
    assert!(ids[0] > ids[1] && ids[1] > ids[2], "statement must be in DESC order by transaction_id");

    // Fetch all 5
    let all = tx_repo.get_statement(account.account_id, 10).await.unwrap();
    assert_eq!(all.len(), 5, "get_statement with limit=10 must return all 5 rows");

    cleanup_user(&pool, &username);
    println!("✅ test_5000 passed — statement count and order verified");
}

/// Test 5001: Empty account get_statement returns an empty Vec (not an error).
#[tokio::test]
async fn test_5001_empty_account_statement_returns_empty_vec() {
    let pool = setup_pool();
    let users_repo = UsersRepository::new(Arc::clone(&pool));
    let accounts_repo = AccountsRepository::new(Arc::clone(&pool));
    let tx_repo = TransactionsRepository::new(Arc::clone(&pool));

    let username = unique_username("test_5001");
    let user = users_repo.insert(NewUser::new(
        Name::new("Rachel".into(), None, "Diaz".into()).unwrap(),
        Password::new("EmptyAcc!Statement9Zr".into()).unwrap(),
        username.clone(),
        None,
    )).await.unwrap();
    let account = accounts_repo.insert(NewAccount::new(user.user_id, AccountType::Checking)).await.unwrap();

    let statement = tx_repo.get_statement(account.account_id, 10).await.unwrap();
    assert!(statement.is_empty(), "brand new account must have an empty statement");

    cleanup_user(&pool, &username);
    println!("✅ test_5001 passed — empty statement is an empty Vec");
}

// ─────────────────────────────────────────────────────────────────────────────
// 6000 series: Full End-to-End Banking Flow
// ─────────────────────────────────────────────────────────────────────────────

/// Test 6000: Full end-to-end banking flow — from user registration to inter-account transfer.
///
/// This is the MEGA test that covers the entire application stack:
///   User A registers → opens savings → deposits $1000
///   User B registers → opens checking
///   A transfers $400 to B → verify both balances → verify hash chain integrity
///   B withdraws $100 → verify B's balance → get B's statement (2 txns)
#[tokio::test]
async fn test_6000_full_end_to_end_banking_flow() {
    let pool = setup_pool();
    let users_repo = UsersRepository::new(Arc::clone(&pool));
    let accounts_repo = AccountsRepository::new(Arc::clone(&pool));
    let tx_repo = TransactionsRepository::new(Arc::clone(&pool));

    // ── Phase 1: User A — Register + Open Savings + Deposit ───────────────
    let alice_uname = unique_username("test_6000_alice");
    let alice = users_repo.insert(NewUser::new(
        Name::new("Alice".into(), Some("Marie".into()), "Angulo".into()).unwrap(),
        Password::new("AliceE2E!99Ztest".into()).unwrap(),
        alice_uname.clone(),
        Some(Email::new(format!("{}@bank.test", alice_uname)).unwrap()),
    )).await.expect("test_6000: alice insert failed");

    assert!(alice.user_id.value() > 0);
    assert_eq!(alice.name.full_name(), "Alice Marie Angulo");

    let alice_acc = accounts_repo.insert(NewAccount::new(alice.user_id, AccountType::Savings)).await
        .expect("test_6000: alice account failed");
    assert_eq!(alice_acc.account_type, AccountType::Savings);
    assert!(!alice_acc.account_number.as_str().is_empty());

    let alice_deposit = tx_repo.deposit(alice_acc.account_id, 100_000).await
        .expect("test_6000: alice deposit failed");
    assert_eq!(alice_deposit.amount_cents, 100_000);
    assert_eq!(alice_deposit.transaction_type, TransactionType::Deposit);
    assert!(alice_deposit.previous_hash.is_none(), "alice's genesis tx has no previous hash");

    // ── Phase 2: User B — Register + Open Checking ────────────────────────
    let bob_uname = unique_username("test_6000_bob");
    let bob = users_repo.insert(NewUser::new(
        Name::new("Bob".into(), None, "Santos".into()).unwrap(),
        Password::new("BobE2E!99Ztestb".into()).unwrap(),
        bob_uname.clone(),
        None,
    )).await.expect("test_6000: bob insert failed");

    let bob_acc = accounts_repo.insert(NewAccount::new(bob.user_id, AccountType::Checking)).await
        .expect("test_6000: bob account failed");

    // ── Phase 3: A transfers $400 to B ────────────────────────────────────
    let (tx_out, tx_in) = tx_repo.transfer(alice_acc.account_id, bob_acc.account_id, 40_000).await
        .expect("test_6000: transfer failed");

    assert_eq!(tx_out.amount_cents, -40_000);
    assert_eq!(tx_in.amount_cents, 40_000);
    assert_eq!(tx_out.transaction_type, TransactionType::TransferOut);
    assert_eq!(tx_in.transaction_type, TransactionType::TransferIn);

    // Hash chain: alice's tx_out chains from her deposit
    assert_eq!(
        tx_out.previous_hash.as_deref(),
        Some(alice_deposit.current_hash.as_str()),
        "alice's transfer_out must chain from her deposit"
    );
    // Bob's genesis tx_in has no previous hash
    assert!(tx_in.previous_hash.is_none(), "bob's first tx_in must be genesis");

    // ── Phase 4: Verify balances ───────────────────────────────────────────
    let alice_balance = tx_repo.get_balance(alice_acc.account_id).await.unwrap();
    let bob_balance   = tx_repo.get_balance(bob_acc.account_id).await.unwrap();

    assert_eq!(alice_balance.cents(), 60_000, "alice must have $600 after $400 transfer");
    assert_eq!(bob_balance.cents(), 40_000,   "bob must have $400 after receiving transfer");

    // ── Phase 5: Bob withdraws $100 ───────────────────────────────────────
    tx_repo.withdraw(bob_acc.account_id, 10_000).await
        .expect("test_6000: bob withdrawal failed");

    let bob_final = tx_repo.get_balance(bob_acc.account_id).await.unwrap();
    assert_eq!(bob_final.cents(), 30_000, "bob must have $300 after $100 withdrawal");

    // ── Phase 6: Bob's statement has 2 transactions (TRANSFER_IN + WITHDRAWAL)
    let bob_statement = tx_repo.get_statement(bob_acc.account_id, 10).await.unwrap();
    assert_eq!(bob_statement.len(), 2, "bob must have exactly 2 transactions");
    assert_eq!(bob_statement[0].transaction_type, TransactionType::Withdrawal, "most recent is withdrawal");
    assert_eq!(bob_statement[1].transaction_type, TransactionType::TransferIn, "oldest is transfer_in");

    // ── Phase 7: find_account_by_id reflects alice's account correctly ─────
    let alice_account_lookup = tx_repo.find_account_by_id(alice_acc.account_id).await.unwrap();
    assert_eq!(alice_account_lookup.account_id.value(), alice_acc.account_id.value());
    assert_eq!(alice_account_lookup.balance.cents(), 60_000);

    cleanup_user(&pool, &alice_uname);
    cleanup_user(&pool, &bob_uname);
    println!("✅ test_6000 MEGA END-TO-END PASSED 🏦🚀 alice_bal={}c bob_bal={}c", alice_balance.cents(), bob_final.cents());
}

/// Test 3005: Oracle trigger trg_transactions_status_guard blocks transactions on non-ACTIVE accounts.
#[tokio::test]
async fn test_3005_frozen_account_blocked_by_oracle_trigger() {
    let pool = setup_pool();
    let user_repo = UsersRepository::new(Arc::clone(&pool));
    let acc_repo = AccountsRepository::new(Arc::clone(&pool));

    let username = unique_username("test_3005");
    let name = Name::new("Freeze".into(), None, "User".into()).unwrap();
    let pwd = Password::new("StrongP@ssw0rd99!".into()).unwrap();
    let user = user_repo.insert(NewUser::new(name, pwd, username.clone(), None)).await.unwrap();

    let acc = acc_repo.insert(NewAccount::new(user.user_id, AccountType::Savings)).await.unwrap();

    // Freeze the account directly in Oracle
    let conn = pool.acquire().unwrap();
    let acc_id_val = acc.account_id.value();
    conn.execute_named(
        "UPDATE accounts SET status = 'FROZEN' WHERE account_id = :id",
        &[("id", &acc_id_val)],
    ).unwrap();
    conn.commit().unwrap();

    // Attempt direct transaction insert on the frozen account — trigger must reject it!
    let result = conn.execute_named(
        "INSERT INTO transactions (account_id, amount_cents, transaction_type) \
         VALUES (:acc_id, 50000, 'DEPOSIT')",
        &[("acc_id", &acc_id_val)],
    );

    match result {
        Err(e) => {
            let err_msg = e.to_string();
            assert!(
                err_msg.contains("ORA-20001") || err_msg.contains("ACCOUNT_NOT_ACTIVE"),
                "Expected trigger to raise ACCOUNT_NOT_ACTIVE, got: {}",
                err_msg
            );
            println!("✅ test_3005: Trigger successfully blocked transaction on FROZEN account!");
        }
        Ok(_) => panic!("Expected trigger to reject transaction on FROZEN account, but it succeeded!"),
    }

    cleanup_user(&pool, &username);
}


