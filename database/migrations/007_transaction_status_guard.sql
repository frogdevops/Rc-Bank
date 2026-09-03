-- Up migration

CREATE OR REPLACE TRIGGER trg_transactions_status_guard
    BEFORE INSERT ON transactions
    FOR EACH ROW
DECLARE
    v_status VARCHAR2(20);
BEGIN
    SELECT status INTO v_status
    FROM accounts
    WHERE account_id = :new.account_id;
    IF v_status != 'ACTIVE' THEN
        RAISE_APPLICATION_ERROR(-20001, 'ACCOUNT_NOT_ACTIVE: Account status is ' || v_status);
    END IF;
END;
/

-- Down migration
DROP TRIGGER trg_transactions_status_guard;