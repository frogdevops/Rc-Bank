-- Up migration

CREATE TABLE refresh_tokens (
    token_id NUMBER GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    user_id NUMBER NOT NULL,
    token_hash VARCHAR2(64) UNIQUE NOT NULL,
    device_info VARCHAR2(100),
    is_verified NUMBER(1) DEFAULT 1 NOT NULL,
    is_revoked NUMBER(1) DEFAULT 0 NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP NOT NULL,
    last_used_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP NOT NULL,

    CONSTRAINT fk_refresh_token_user
        FOREIGN KEY (user_id)
        REFERENCES users (user_id)
        ON DELETE CASCADE,

    CONSTRAINT chk_token_verified
        CHECK (is_verified IN (0, 1)),

    CONSTRAINT chk_token_revoked
        CHECK (is_revoked IN (0, 1))
);

-- Down migration

DROP TABLE refresh_tokens CASCADE CONSTRAINTS;
