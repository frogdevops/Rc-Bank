-- Up migration

CREATE SEQUENCE account_number_seq
    START WITH 1
    INCREMENT BY 1
    NOCACHE;

CREATE OR REPLACE FUNCTION generate_account_number
    RETURN VARCHAR2
    IS
    v_seq   NUMBER;
    v_raw   RAW(32);
    v_hex   VARCHAR2(64);
    v_num   NUMBER;
BEGIN
    v_seq := account_number_seq.NEXTVAL;

    -- hash the sequence value + a fixed app-level constant
    -- noinspection SqlResolve
    v_raw := SYS.DBMS_CRYPTO.HASH(
            UTL_I18N.STRING_TO_RAW(TO_CHAR(v_seq) || 'ZKUJvA32z3s++6PQp7XRU9DWdph8Gi3+70w9Er5EmTU=', 'AL32UTF8'),
            SYS.DBMS_CRYPTO.HASH_SH256
             );

    v_hex := RAWTOHEX(v_raw);

    -- take the first 16 hex chars (64 bits) and convert to a decimal number
    v_num := TO_NUMBER(SUBSTR(v_hex, 1, 15), 'XXXXXXXXXXXXXXX');

    -- format as a fixed 20-digit, zero-padded string
    RETURN LPAD(TO_CHAR(MOD(v_num, POWER(10, 20))), 20, '0');
END;
/

-- Down migration

DROP FUNCTION generate_account_number;
DROP SEQUENCE account_number_seq;