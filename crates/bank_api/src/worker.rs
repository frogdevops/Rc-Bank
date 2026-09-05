use std::sync::Arc;
use std::time::Duration;
use futures::StreamExt;
use async_nats::jetstream::{self, AckKind};
use async_nats::jetstream::consumer::pull;
use bank_domain::{AccountID, AccountNumber, Amount, DepositCommand, MoneyResult, TransferCommand, TransferResult, WithdrawCommand};
use crate::services::TransactionsService;

/// Creates a JetStream durable pull consumer for the given stream and consumer name.
async fn make_pull_consumer(
    nats_client: &async_nats::Client,
    stream_name: &str,
    consumer_name: &str,
) -> Result<jetstream::consumer::Consumer<pull::Config>, async_nats::Error> {
    let js = jetstream::new(nats_client.clone());
    let stream = js.get_stream(stream_name).await?;
    let consumer = stream
        .get_or_create_consumer(
            consumer_name,
            pull::Config {
                durable_name: Some(consumer_name.to_string()),
                ack_policy: async_nats::jetstream::consumer::AckPolicy::Explicit,
                ack_wait: Duration::from_secs(30),
                max_deliver: 5, // retry up to 5 times on transient failures
                ..Default::default()
            },
        )
        .await?;
    Ok(consumer)
}

/// Maps a debug-formatted TransactionError string back to a typed error variant.
///
/// ACK strategy:
/// - Business errors (InsufficientFunds, AccountNotFound, etc.) → ACK + reply with error
///   (no point retrying — the answer won't change)
/// - Transient DB errors → NAK + reply with error
///   (worker crashed or DB blipped; NATS will redeliver, client gets a timeout error)
fn is_transient_error(err_str: &str) -> bool {
    err_str.contains("DatabaseError")
}

/// Starts the JetStream durable pull consumer worker for transfer operations.
///
/// Listens on stream `BANK_TRANSFERS`, consumer `transfer_worker`.
/// ACKs on success and business errors. NAKs on transient DB errors.
pub async fn start_transfer_worker(
    nats_client: async_nats::Client,
    tx_service: Arc<TransactionsService>,
) -> Result<(), async_nats::Error> {
    let consumer = make_pull_consumer(&nats_client, "BANK_TRANSFERS", "transfer_worker").await?;
    let mut messages = consumer.messages().await?;

    println!(" [NATS Worker] Transfer worker ready — durable consumer 'transfer_worker' on BANK_TRANSFERS");

    while let Some(msg_result) = messages.next().await {
        let msg = match msg_result {
            Ok(m) => m,
            Err(e) => {
                eprintln!("❌ [Transfer Worker] Message stream error: {:?}", e);
                continue;
            }
        };

        let tx_service = tx_service.clone();
        let nats = nats_client.clone();

        tokio::spawn(async move {
            // Deserialize — ACK malformed messages (retrying won't help)
            let command: TransferCommand = match serde_json::from_slice(&msg.payload) {
                Ok(cmd) => cmd,
                Err(e) => {
                    eprintln!("❌ [Transfer Worker] Malformed payload, terminating: {:?}", e);
                    if let Some(reply) = &msg.reply {
                        let err_res = TransferResult::err("UNKNOWN".to_string(), format!("Invalid payload: {}", e));
                        if let Ok(bytes) = serde_json::to_vec(&err_res) {
                            let _ = nats.publish(reply.clone(), bytes.into()).await;
                        }
                    }
                    let _ = msg.ack_with(AckKind::Term).await; // Term = don't redeliver garbage
                    return;
                }
            };

            let cid = command.correlation_id.clone();
            println!(" [Transfer Worker] [CID: {}] from:{} to:{} amount:{}",
                cid, command.from_account_id.value(), command.to_account_number, command.amount_cents
            );

            let reply_target = command.reply_to.clone()
                .or_else(|| msg.reply.as_ref().map(|s| s.to_string()));

            let amount = match Amount::new(command.amount_cents) {
                Ok(a) => a,
                Err(e) => {
                    let err_res = TransferResult::err(cid, format!("{:?}", e));
                    send_reply(&nats, &reply_target, &err_res).await;
                    let _ = msg.ack().await; // business validation failure — don't retry
                    return;
                }
            };

            let result = tx_service
                .transfer(
                    command.user_id,
                    command.from_account_id,
                    AccountNumber::from_db(command.to_account_number.to_string()),
                    amount,
                )
                .await;

            match result {
                Ok((debit_tx, credit_tx)) => {
                    println!(" [Transfer Worker] ✅ [CID: {}] debit_hash:{}", cid, debit_tx.current_hash);
                    let response = TransferResult::ok(cid, debit_tx, credit_tx);
                    send_reply(&nats, &reply_target, &response).await;
                    let _ = msg.ack().await;
                }
                Err(e) => {
                    let err_str = format!("{:?}", e);
                    eprintln!("⚠️ [Transfer Worker] [CID: {}] rejected: {}", cid, err_str);
                    let response = TransferResult::err(cid, &err_str);
                    send_reply(&nats, &reply_target, &response).await;
                    if is_transient_error(&err_str) {
                        let _ = msg.ack_with(AckKind::Nak(Some(Duration::from_secs(2)))).await;
                    } else {
                        let _ = msg.ack().await; // business error — ACK, don't redeliver
                    }
                }
            }
        });
    }

    Ok(())
}

/// Starts the JetStream durable pull consumer worker for deposit operations.
///
/// Listens on stream `BANK_DEPOSITS`, consumer `deposit_worker`.
pub async fn start_deposit_worker(
    nats_client: async_nats::Client,
    tx_service: Arc<TransactionsService>,
) -> Result<(), async_nats::Error> {
    let consumer = make_pull_consumer(&nats_client, "BANK_DEPOSITS", "deposit_worker").await?;
    let mut messages = consumer.messages().await?;

    println!(" [NATS Worker] Deposit worker ready — durable consumer 'deposit_worker' on BANK_DEPOSITS");

    while let Some(msg_result) = messages.next().await {
        let msg = match msg_result {
            Ok(m) => m,
            Err(e) => {
                eprintln!("❌ [Deposit Worker] Message stream error: {:?}", e);
                continue;
            }
        };

        let tx_service = tx_service.clone();
        let nats = nats_client.clone();

        tokio::spawn(async move {
            let command: DepositCommand = match serde_json::from_slice(&msg.payload) {
                Ok(cmd) => cmd,
                Err(e) => {
                    eprintln!("❌ [Deposit Worker] Malformed payload, terminating: {:?}", e);
                    if let Some(reply) = &msg.reply {
                        let err_res = MoneyResult::err("UNKNOWN".to_string(), format!("Invalid payload: {}", e));
                        if let Ok(bytes) = serde_json::to_vec(&err_res) {
                            let _ = nats.publish(reply.clone(), bytes.into()).await;
                        }
                    }
                    let _ = msg.ack_with(AckKind::Term).await;
                    return;
                }
            };

            let cid = command.correlation_id.clone();
            println!(" [Deposit Worker] [CID: {}] account:{} amount:{}", cid, command.account_id.value(), command.amount_cents);

            let reply_target = command.reply_to.clone()
                .or_else(|| msg.reply.as_ref().map(|s| s.to_string()));

            let amount = match Amount::new(command.amount_cents) {
                Ok(a) => a,
                Err(e) => {
                    let err_res = MoneyResult::err(cid, format!("{:?}", e));
                    send_money_reply(&nats, &reply_target, &err_res).await;
                    let _ = msg.ack().await;
                    return;
                }
            };

            let result = tx_service
                .deposit(command.user_id, AccountID::from_db(command.account_id.value()), amount)
                .await;

            match result {
                Ok(tx) => {
                    println!(" [Deposit Worker] ✅ [CID: {}] hash:{}", cid, tx.current_hash);
                    let response = MoneyResult::ok(cid, tx);
                    send_money_reply(&nats, &reply_target, &response).await;
                    let _ = msg.ack().await;
                }
                Err(e) => {
                    let err_str = format!("{:?}", e);
                    eprintln!("⚠️ [Deposit Worker] [CID: {}] rejected: {}", cid, err_str);
                    let response = MoneyResult::err(cid, &err_str);
                    send_money_reply(&nats, &reply_target, &response).await;
                    if is_transient_error(&err_str) {
                        let _ = msg.ack_with(AckKind::Nak(Some(Duration::from_secs(2)))).await;
                    } else {
                        let _ = msg.ack().await;
                    }
                }
            }
        });
    }

    Ok(())
}

/// Starts the JetStream durable pull consumer worker for withdrawal operations.
///
/// Listens on stream `BANK_WITHDRAWALS`, consumer `withdraw_worker`.
pub async fn start_withdraw_worker(
    nats_client: async_nats::Client,
    tx_service: Arc<TransactionsService>,
) -> Result<(), async_nats::Error> {
    let consumer = make_pull_consumer(&nats_client, "BANK_WITHDRAWALS", "withdraw_worker").await?;
    let mut messages = consumer.messages().await?;

    println!(" [NATS Worker] Withdraw worker ready — durable consumer 'withdraw_worker' on BANK_WITHDRAWALS");

    while let Some(msg_result) = messages.next().await {
        let msg = match msg_result {
            Ok(m) => m,
            Err(e) => {
                eprintln!("❌ [Withdraw Worker] Message stream error: {:?}", e);
                continue;
            }
        };

        let tx_service = tx_service.clone();
        let nats = nats_client.clone();

        tokio::spawn(async move {
            let command: WithdrawCommand = match serde_json::from_slice(&msg.payload) {
                Ok(cmd) => cmd,
                Err(e) => {
                    eprintln!("❌ [Withdraw Worker] Malformed payload, terminating: {:?}", e);
                    if let Some(reply) = &msg.reply {
                        let err_res = MoneyResult::err("UNKNOWN".to_string(), format!("Invalid payload: {}", e));
                        if let Ok(bytes) = serde_json::to_vec(&err_res) {
                            let _ = nats.publish(reply.clone(), bytes.into()).await;
                        }
                    }
                    let _ = msg.ack_with(AckKind::Term).await;
                    return;
                }
            };

            let cid = command.correlation_id.clone();
            println!(" [Withdraw Worker] [CID: {}] account:{} amount:{}", cid, command.account_id.value(), command.amount_cents);

            let reply_target = command.reply_to.clone()
                .or_else(|| msg.reply.as_ref().map(|s| s.to_string()));

            let amount = match Amount::new(command.amount_cents) {
                Ok(a) => a,
                Err(e) => {
                    let err_res = MoneyResult::err(cid, format!("{:?}", e));
                    send_money_reply(&nats, &reply_target, &err_res).await;
                    let _ = msg.ack().await;
                    return;
                }
            };

            let result = tx_service
                .withdraw(command.user_id, AccountID::from_db(command.account_id.value()), amount)
                .await;

            match result {
                Ok(tx) => {
                    println!(" [Withdraw Worker] ✅ [CID: {}] hash:{}", cid, tx.current_hash);
                    let response = MoneyResult::ok(cid, tx);
                    send_money_reply(&nats, &reply_target, &response).await;
                    let _ = msg.ack().await;
                }
                Err(e) => {
                    let err_str = format!("{:?}", e);
                    eprintln!("⚠️ [Withdraw Worker] [CID: {}] rejected: {}", cid, err_str);
                    let response = MoneyResult::err(cid, &err_str);
                    send_money_reply(&nats, &reply_target, &response).await;
                    if is_transient_error(&err_str) {
                        let _ = msg.ack_with(AckKind::Nak(Some(Duration::from_secs(2)))).await;
                    } else {
                        let _ = msg.ack().await;
                    }
                }
            }
        });
    }

    Ok(())
}

// ── Helpers ──────────────────────────────────────────────────────────────────

async fn send_reply(nats: &async_nats::Client, target: &Option<String>, payload: &TransferResult) {
    if let Some(subject) = target {
        if let Ok(bytes) = serde_json::to_vec(payload) {
            if let Err(e) = nats.publish(subject.clone(), bytes.into()).await {
                eprintln!("❌ [Transfer Worker] Failed to send reply: {:?}", e);
            }
        }
    }
}

async fn send_money_reply(nats: &async_nats::Client, target: &Option<String>, payload: &MoneyResult) {
    if let Some(subject) = target {
        if let Ok(bytes) = serde_json::to_vec(payload) {
            if let Err(e) = nats.publish(subject.clone(), bytes.into()).await {
                eprintln!("❌ [Money Worker] Failed to send reply: {:?}", e);
            }
        }
    }
}
