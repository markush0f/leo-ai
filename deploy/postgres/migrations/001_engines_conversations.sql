CREATE TABLE IF NOT EXISTS schema_migrations (
    version INT PRIMARY KEY,
    applied_at TIMESTAMPTZ NOT NULL DEFAULT now()
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
    channel TEXT NOT NULL CHECK (channel IN ('local', 'telegram', 'voice')),
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

CREATE TABLE IF NOT EXISTS secrets (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

ALTER TABLE settings ADD COLUMN IF NOT EXISTS voice_system_prompt TEXT NOT NULL DEFAULT 'Eres Leo, un asistente de voz. Responde en español, breve y claro, para ser leído en voz alta.';
ALTER TABLE settings ADD COLUMN IF NOT EXISTS stt_engine_id UUID REFERENCES engines (id) ON DELETE SET NULL;
ALTER TABLE settings ADD COLUMN IF NOT EXISTS tts_engine_id UUID REFERENCES engines (id) ON DELETE SET NULL;
ALTER TABLE settings ADD COLUMN IF NOT EXISTS wake_engine_id UUID REFERENCES engines (id) ON DELETE SET NULL;
ALTER TABLE settings ADD COLUMN IF NOT EXISTS audio_source TEXT NOT NULL DEFAULT '@DEFAULT_SOURCE@';
ALTER TABLE settings ADD COLUMN IF NOT EXISTS audio_sink TEXT NOT NULL DEFAULT '@DEFAULT_SINK@';
ALTER TABLE settings ADD COLUMN IF NOT EXISTS vad_hangover_ms INT NOT NULL DEFAULT 500;
ALTER TABLE settings ADD COLUMN IF NOT EXISTS barge_in BOOLEAN NOT NULL DEFAULT TRUE;
ALTER TABLE settings ADD COLUMN IF NOT EXISTS barge_in_rms REAL NOT NULL DEFAULT 0.035;
ALTER TABLE settings ADD COLUMN IF NOT EXISTS stt_language TEXT NOT NULL DEFAULT 'es';
ALTER TABLE settings ADD COLUMN IF NOT EXISTS thinking BOOLEAN NOT NULL DEFAULT FALSE;
ALTER TABLE settings ADD COLUMN IF NOT EXISTS tools_enabled BOOLEAN NOT NULL DEFAULT TRUE;
ALTER TABLE settings ADD COLUMN IF NOT EXISTS active_conversation_id UUID REFERENCES conversations (id) ON DELETE SET NULL;
ALTER TABLE settings ADD COLUMN IF NOT EXISTS telegram_token TEXT;
ALTER TABLE settings ADD COLUMN IF NOT EXISTS telegram_allow_users BIGINT[] NOT NULL DEFAULT '{}';

INSERT INTO engines (id, role, kind, name, provider_id) VALUES
    ('00000000-0000-4000-8000-000000000501', 'stt', 'grok', 'grok-stt', '00000000-0000-4000-8000-000000000001'),
    ('00000000-0000-4000-8000-000000000502', 'stt', 'null', 'none', NULL),
    ('00000000-0000-4000-8000-000000000601', 'tts', 'null', 'tone', NULL),
    ('00000000-0000-4000-8000-000000000701', 'wake', 'noop', 'none', NULL)
ON CONFLICT (id) DO NOTHING;

UPDATE settings SET
    stt_engine_id = COALESCE(stt_engine_id, '00000000-0000-4000-8000-000000000501'),
    tts_engine_id = COALESCE(tts_engine_id, '00000000-0000-4000-8000-000000000601'),
    wake_engine_id = COALESCE(wake_engine_id, '00000000-0000-4000-8000-000000000701')
WHERE id = 1;
