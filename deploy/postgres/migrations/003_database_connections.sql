CREATE TABLE IF NOT EXISTS database_connections (
    id UUID PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    host TEXT NOT NULL,
    port INT NOT NULL CHECK (port BETWEEN 1 AND 65535),
    database_name TEXT NOT NULL,
    username TEXT NOT NULL,
    ssl_mode TEXT NOT NULL DEFAULT 'prefer'
        CHECK (ssl_mode IN ('disable', 'prefer', 'require', 'verify-ca', 'verify-full')),
    enabled BOOLEAN NOT NULL DEFAULT FALSE,
    password_ciphertext BYTEA,
    password_nonce BYTEA,
    last_test_ok BOOLEAN,
    last_test_error TEXT,
    last_tested_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK ((password_ciphertext IS NULL) = (password_nonce IS NULL))
);
