use std::sync::Arc;
use chrono::Utc;
use futures::StreamExt;
use bank_api::nats::connect_nats;
use bank_api::services::{AccountsService, TransactionsService, UsersService};
use bank_api::worker::start_transfer_worker;
use bank_db::{
    create_oracle_pool, AccountsRepository, TransactionsRepository, UsersRepository,
};
use bank_domain::{
    AccountType, Amount, TransferCommand, TransferResult,
};

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
        "DELETE FROM users WHERE user_name = :u",
        &[("u", &user_name)],
    ).ok();
    conn.commit().ok();
}

#[tokio::test]
async fn test_nats_worker_transfer_end_to_end() {
    let pool = setup_pool();
    let users_repo = UsersRepository::new(Arc::clone(&pool));
    let accounts_repo = AccountsRepository::new(Arc::clone(&pool));
    let tx_repo = TransactionsRepository::new(Arc::clone(&pool));

    let users_service = UsersService::new(users_repo.clone());
    let accounts_service = AccountsService::new(users_repo, accounts_repo);
    let tx_service = Arc::new(TransactionsService::new(tx_repo.clone()));

    // 1. Connect to NATS
    let nats_client = connect_nats("127.0.0.1:4222")
        .await
        .expect("Failed to connect to NATS");

    // 2. Spawn the worker in background
    let worker_nats = nats_client.clone();
    let worker_tx = Arc::clone(&tx_service);
    tokio::spawn(async move {
        if let Err(e) = start_transfer_worker(worker_nats, worker_tx).await {
            eprintln!("Worker error: {:?}", e);
        }
    });

    // Small delay to ensure queue subscription is active
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // 3. Set up test accounts in Oracle
    let sender_name = unique_username("nats_sender");
    let recip_name = unique_username("nats_recip");

    let sender = users_service.create_user(
        "Alice".into(), None, "Sender".into(), sender_name.clone(), "Str0ngPass!99".into(), None,
    ).await.unwrap();
    let recip = users_service.create_user(
        "Bob".into(), None, "Receiver".into(), recip_name.clone(), "Str0ngPass!99".into(), None,
    ).await.unwrap();

    let sender_acc = accounts_service.create_account(sender.user_id, AccountType::Checking).await.unwrap();
    let recip_acc = accounts_service.create_account(recip.user_id, AccountType::Savings).await.unwrap();

    // Deposit $500.00 (50,000 cents) into Alice's account
    tx_service.deposit(sender.user_id, sender_acc.account_id, Amount::new(50_000).unwrap())
        .await
        .unwrap();

    // 4. Send Transfer Request over NATS (Alice -> Bob $200.00)
    let inbox = nats_client.new_inbox();
    let mut reply_sub = nats_client.subscribe(inbox.clone()).await.unwrap();

    let command = TransferCommand {
        correlation_id: "test-cid-1001".to_string(),
        reply_to: Some(inbox),
        user_id: sender.user_id,
        from_account_id: sender_acc.account_id,
        to_account_number: recip_acc.account_number.clone(),
        amount_cents: 20_000,
        created_at: Utc::now(),
    };

    let payload = serde_json::to_vec(&command).unwrap();
    nats_client
        .publish("bank.transfers", payload.into())
        .await
        .expect("NATS publish failed");

    let reply_msg = tokio::time::timeout(tokio::time::Duration::from_secs(5), reply_sub.next())
        .await
        .expect("Timed out waiting for worker reply")
        .expect("Expected reply message");

    let transfer_result: TransferResult = serde_json::from_slice(&reply_msg.payload)
        .expect("Failed to deserialize TransferResult");

    // 5. Verify the result
    assert!(transfer_result.success, "Transfer should succeed");
    assert_eq!(transfer_result.correlation_id, "test-cid-1001");
    let debit = transfer_result.debit_transaction.expect("Debit tx present");
    let credit = transfer_result.credit_transaction.expect("Credit tx present");

    assert_eq!(debit.amount_cents, -20_000);
    assert_eq!(credit.amount_cents, 20_000);
    assert!(!debit.current_hash.is_empty(), "Debit transaction has SHA-256 hash");
    assert!(!credit.current_hash.is_empty(), "Credit transaction has SHA-256 hash");

    // 6. Verify balances in Oracle directly
    let sender_bal = tx_service.get_balance(sender.user_id, sender_acc.account_id).await.unwrap();
    let recip_bal = tx_service.get_balance(recip.user_id, recip_acc.account_id).await.unwrap();
    assert_eq!(sender_bal.cents(), 30_000); // $300.00 left
    assert_eq!(recip_bal.cents(), 20_000);  // $200.00 received

    println!("✅ NATS successful transfer verified! Debit hash: {}", debit.current_hash);

    // 7. Test Insufficient Funds over NATS (Alice tries to send $900 with only $300)
    let inbox2 = nats_client.new_inbox();
    let mut reply_sub2 = nats_client.subscribe(inbox2.clone()).await.unwrap();

    let bad_cmd = TransferCommand {
        correlation_id: "test-cid-1002".to_string(),
        reply_to: Some(inbox2),
        user_id: sender.user_id,
        from_account_id: sender_acc.account_id,
        to_account_number: recip_acc.account_number.clone(),
        amount_cents: 90_000,
        created_at: Utc::now(),
    };
    let bad_payload = serde_json::to_vec(&bad_cmd).unwrap();
    nats_client
        .publish("bank.transfers", bad_payload.into())
        .await
        .expect("NATS publish failed");

    let bad_reply = tokio::time::timeout(tokio::time::Duration::from_secs(5), reply_sub2.next())
        .await
        .expect("Timed out waiting for worker reply")
        .expect("Expected reply message");

    let bad_result: TransferResult = serde_json::from_slice(&bad_reply.payload).unwrap();
    assert!(!bad_result.success, "Transfer should be rejected due to insufficient funds");
    assert!(bad_result.error_message.unwrap().contains("InsufficientFunds"));

    println!("✅ NATS Insufficient Funds rejection verified cleanly!");

    // Cleanup
    cleanup_user(&pool, &sender_name);
    cleanup_user(&pool, &recip_name);
}
