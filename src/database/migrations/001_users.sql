-- Up migration

CREATE TABLE users (
       user_id NUMBER GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
       first_name VARCHAR2(100) NOT NULL,
       middle_name VARCHAR2(50),
       last_name VARCHAR2(100) NOT NULL,
       email VARCHAR2(50),
       user_name VARCHAR(50) NOT NULL UNIQUE,
       password_hash VARCHAR(255) NOT NULL,
       refresh_token VARCHAR(64),
       created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP NOT NULL,
       updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP NOT NULL
);

-- Down migration

DROP TABLE users;