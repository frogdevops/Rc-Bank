-- Up migration
CREATE INDEX idx_accounts_user_id ON accounts (user_id);
CREATE INDEX idx_transactions_account_id ON transactions (account_id);

-- Down migration
DROP INDEX idx_transactions_account_id;
DROP INDEX idx_accounts_user_id;
