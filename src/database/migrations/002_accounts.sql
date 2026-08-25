-- Up migration

CREATE TABLE accounts (
    account_id NUMBER GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    account_number VARCHAR(20) UNIQUE NOT NULL,
    user_id NUMBER NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP NOT NULL,
    balance NUMBER(15,2) DEFAULT 0 NOT NULL,
    status VARCHAR(20) DEFAULT 'ACTIVE' NOT NULL,

        CONSTRAINT fk_account_user
        FOREIGN KEY (user_id)
        REFERENCES users(USER_ID),

        CONSTRAINT chk_account_status
        CHECK (
            status IN (
                'ACTIVE',
                'FROZEN',
                'CLOSED',
                'SUSPENDED'
            )
        )
);

-- Down migration
DROP TABLE accounts;