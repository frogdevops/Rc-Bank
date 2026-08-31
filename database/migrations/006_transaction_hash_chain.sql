-- Up migration

CREATE OR REPLACE TRIGGER trg_transactions_hash_chain
    BEFORE INSERT ON transactions
    FOR EACH ROW
DECLARE
    v_prev_hash VARCHAR2(64);
    v_payload   VARCHAR2(500);
BEGIN
    -- 1. Fetch the latest hash for this account (NULL if genesis/first transaction)
    BEGIN
        SELECT current_hash INTO v_prev_hash
        FROM (
                 SELECT current_hash
                 FROM transactions
                 WHERE account_id = :new.account_id
                 ORDER BY transaction_id DESC
             )
        WHERE ROWNUM = 1;
    EXCEPTION
        WHEN NO_DATA_FOUND THEN
            v_prev_hash := NULL;
    END;

    :new.previous_hash := v_prev_hash;

    -- 2. Build cryptographic payload using TM9 (Text-Minimum format model)
    v_payload := NVL(v_prev_hash, 'GENESIS') || '|' ||
                 TO_CHAR(:new.account_id, 'TM9') || '|' ||
                 TO_CHAR(:new.amount_cents, 'TM9') || '|' ||
                 :new.transaction_type;

    -- 3. Convert 32-byte RAW hash to 64-char HEX string
    SELECT RAWTOHEX(STANDARD_HASH(v_payload, 'SHA256'))
    INTO :new.current_hash
    FROM DUAL;
END;
/

-- Down migration

DROP TRIGGER trg_transactions_hash_chain;