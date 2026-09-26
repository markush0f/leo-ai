CREATE TABLE IF NOT EXISTS schema_migrations (
    version INT PRIMARY KEY,
    applied_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS providers (
    id UUID PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    kind TEXT NOT NULL,
    base_url TEXT,
    api_key TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS models (
    id UUID PRIMARY KEY,
    provider_id UUID NOT NULL REFERENCES providers (id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    UNIQUE (provider_id, name)
);

CREATE TABLE IF NOT EXISTS engines (
    id UUID PRIMARY KEY,
    role TEXT NOT NULL CHECK (role IN ('stt', 'tts', 'wake')),
    kind TEXT NOT NULL,
    name TEXT NOT NULL,
    provider_id UUID REFERENCES providers (id) ON DELETE SET NULL,
    config JSONB NOT NULL DEFAULT '{}',
    UNIQUE (role, name)
);

CREATE TABLE IF NOT EXISTS conversations (
    id UUID PRIMARY KEY,
    channel TEXT NOT NULL CHECK (channel IN ('local', 'telegram', 'voice', 'whatsapp')),
    external_id TEXT,
    title TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    archived_at TIMESTAMPTZ
);

CREATE UNIQUE INDEX IF NOT EXISTS conversations_telegram_live
    ON conversations (external_id)
    WHERE channel = 'telegram' AND archived_at IS NULL AND external_id IS NOT NULL;

CREATE UNIQUE INDEX IF NOT EXISTS conversations_voice_live
    ON conversations (channel)
    WHERE channel = 'voice' AND archived_at IS NULL;

CREATE UNIQUE INDEX IF NOT EXISTS conversations_whatsapp_live
    ON conversations (external_id)
    WHERE channel = 'whatsapp' AND archived_at IS NULL AND external_id IS NOT NULL;

CREATE TABLE IF NOT EXISTS messages (
    id UUID PRIMARY KEY,
    conversation_id UUID NOT NULL REFERENCES conversations (id) ON DELETE CASCADE,
    role TEXT NOT NULL CHECK (role IN ('user', 'assistant', 'tool', 'error')),
    content TEXT NOT NULL,
    tool_call_id TEXT,
    name TEXT,
    tool_calls JSONB NOT NULL DEFAULT '[]',
    model_id UUID REFERENCES models (id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS messages_conversation_created
    ON messages (conversation_id, created_at);

CREATE TABLE IF NOT EXISTS settings (
    id SMALLINT PRIMARY KEY DEFAULT 1 CHECK (id = 1),
    active_model_id UUID REFERENCES models (id) ON DELETE SET NULL,
    system_prompt TEXT NOT NULL,
    voice_system_prompt TEXT NOT NULL DEFAULT 'Eres Ira, un asistente de voz. Responde en español, breve y claro, para ser leído en voz alta.',
    stt_engine_id UUID REFERENCES engines (id) ON DELETE SET NULL,
    tts_engine_id UUID REFERENCES engines (id) ON DELETE SET NULL,
    wake_engine_id UUID REFERENCES engines (id) ON DELETE SET NULL,
    audio_source TEXT NOT NULL DEFAULT '@DEFAULT_SOURCE@',
    audio_sink TEXT NOT NULL DEFAULT '@DEFAULT_SINK@',
    vad_hangover_ms INT NOT NULL DEFAULT 500,
    barge_in BOOLEAN NOT NULL DEFAULT TRUE,
    barge_in_rms REAL NOT NULL DEFAULT 0.035,
    stt_language TEXT NOT NULL DEFAULT 'es',
    thinking BOOLEAN NOT NULL DEFAULT FALSE,
    tools_enabled BOOLEAN NOT NULL DEFAULT TRUE,
    active_conversation_id UUID REFERENCES conversations (id) ON DELETE SET NULL,
    telegram_token TEXT,
    telegram_allow_users BIGINT[] NOT NULL DEFAULT '{}'
);

CREATE TABLE IF NOT EXISTS secrets (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

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

CREATE TABLE IF NOT EXISTS host_services (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    required BOOLEAN NOT NULL DEFAULT FALSE,
    position INT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

INSERT INTO host_services (id, name, required, position) VALUES
    ('postgres', 'Postgres', TRUE, 0),
    ('toolbox', 'Toolbox', TRUE, 1),
    ('ira-realtime', 'Realtime', FALSE, 2),
    ('colibri', 'Colibrì', FALSE, 3),
    ('veritas-kanban', 'Veritas', FALSE, 4),
    ('veritas-mcp', 'Veritas MCP', FALSE, 5)
ON CONFLICT (id) DO NOTHING;

INSERT INTO providers (id, name, kind, base_url) VALUES
    ('00000000-0000-4000-8000-000000000001', 'grok', 'grok', 'https://api.x.ai/v1'),
    ('00000000-0000-4000-8000-000000000002', 'gpt', 'gpt', 'https://api.openai.com/v1'),
    ('00000000-0000-4000-8000-000000000003', 'ollama', 'ollama', 'http://127.0.0.1:11434'),
    ('00000000-0000-4000-8000-000000000004', 'claude', 'claude', 'https://api.anthropic.com/v1'),
    ('00000000-0000-4000-8000-000000000005', 'codex', 'codex', 'https://chatgpt.com/backend-api/codex')
ON CONFLICT (id) DO NOTHING;

INSERT INTO models (id, provider_id, name) VALUES
    ('00000000-0000-4000-8000-000000000101', '00000000-0000-4000-8000-000000000001', 'grok-4.6'),
    ('00000000-0000-4000-8000-000000000102', '00000000-0000-4000-8000-000000000001', 'grok-4.5'),
    ('00000000-0000-4000-8000-000000000201', '00000000-0000-4000-8000-000000000002', 'gpt-4.1'),
    ('00000000-0000-4000-8000-000000000401', '00000000-0000-4000-8000-000000000004', 'claude-sonnet-5'),
    ('00000000-0000-4000-8000-000000000501', '00000000-0000-4000-8000-000000000005', 'gpt-5.4')
ON CONFLICT (id) DO NOTHING;

INSERT INTO engines (id, role, kind, name, provider_id) VALUES
    ('00000000-0000-4000-8000-000000000501', 'stt', 'grok', 'grok-stt', '00000000-0000-4000-8000-000000000001'),
    ('00000000-0000-4000-8000-000000000502', 'stt', 'null', 'none', NULL),
    ('00000000-0000-4000-8000-000000000601', 'tts', 'null', 'tone', NULL),
    ('00000000-0000-4000-8000-000000000701', 'wake', 'noop', 'none', NULL)
ON CONFLICT (id) DO NOTHING;

INSERT INTO settings (
    id, active_model_id, system_prompt, voice_system_prompt,
    stt_engine_id, tts_engine_id, wake_engine_id
) VALUES (
    1,
    '00000000-0000-4000-8000-000000000101',
    'Eres Ira, un asistente. Responde en español, claro y directo.',
    'Eres Ira, un asistente de voz. Responde en español, breve y claro, para ser leído en voz alta.',
    '00000000-0000-4000-8000-000000000501',
    '00000000-0000-4000-8000-000000000601',
    '00000000-0000-4000-8000-000000000701'
)
ON CONFLICT (id) DO NOTHING;

INSERT INTO schema_migrations (version) VALUES (1), (2), (3), (4)
ON CONFLICT (version) DO NOTHING;
