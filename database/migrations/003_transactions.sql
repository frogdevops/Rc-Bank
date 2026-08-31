-- Up migrations

CREATE TABLE transactions (
          transaction_id NUMBER GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
          account_id NUMBER NOT NULL,
          amount_cents NUMBER(15,0) NOT NULL,       -- positive = deposit, negative = withdrawal
          transaction_type VARCHAR2(20) NOT NULL,    -- 'DEPOSIT', 'WITHDRAWAL', 'TRANSFER_IN', etc.
          previous_hash VARCHAR2(64),                -- hash of the PRIOR transaction row (NULL for the first one)
          current_hash VARCHAR2(64) NOT NULL,        -- hash of THIS transaction's own data
          created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP NOT NULL,

            CONSTRAINT fk_account_id
            FOREIGN KEY (account_id)
            REFERENCES accounts (ACCOUNT_ID)
                          ON DELETE CASCADE ,


            CONSTRAINT chk_transaction_type
                CHECK (
                    transaction_type IN (
                               'DEPOSIT',
                               'WITHDRAWAL',
                               'TRANSFER_IN',
                               'TRANSFER_OUT'
                            )
                      ),
            CONSTRAINT chk_transaction_amount_type
              CHECK (
                  (transaction_type IN ('DEPOSIT', 'TRANSFER_IN') AND amount_cents > 0)
                      OR
                  (transaction_type IN ('WITHDRAWAL', 'TRANSFER_OUT') AND amount_cents < 0)
                  )
);
-- Down migrations

DROP TABLE transactions CASCADE CONSTRAINTS;