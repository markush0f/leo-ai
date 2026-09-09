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

CREATE TABLE IF NOT EXISTS settings (
    id SMALLINT PRIMARY KEY DEFAULT 1 CHECK (id = 1),
    active_model_id UUID REFERENCES models (id) ON DELETE SET NULL,
    system_prompt TEXT NOT NULL
);

INSERT INTO providers (id, name, kind, base_url) VALUES
    ('00000000-0000-4000-8000-000000000001', 'grok', 'grok', 'https://api.x.ai/v1'),
    ('00000000-0000-4000-8000-000000000002', 'gpt', 'gpt', 'https://api.openai.com/v1'),
    ('00000000-0000-4000-8000-000000000003', 'ollama', 'ollama', 'http://127.0.0.1:11434'),
    ('00000000-0000-4000-8000-000000000004', 'claude', 'claude', 'https://api.anthropic.com/v1')
ON CONFLICT (id) DO NOTHING;

INSERT INTO models (id, provider_id, name) VALUES
    ('00000000-0000-4000-8000-000000000101', '00000000-0000-4000-8000-000000000001', 'grok-4.6'),
    ('00000000-0000-4000-8000-000000000102', '00000000-0000-4000-8000-000000000001', 'grok-4.5'),
    ('00000000-0000-4000-8000-000000000201', '00000000-0000-4000-8000-000000000002', 'gpt-4.1'),
    ('00000000-0000-4000-8000-000000000401', '00000000-0000-4000-8000-000000000004', 'claude-sonnet-5')
ON CONFLICT (id) DO NOTHING;

INSERT INTO settings (id, active_model_id, system_prompt) VALUES
    (1, '00000000-0000-4000-8000-000000000101', 'Eres Leo, un asistente. Responde en español, claro y directo.')
ON CONFLICT (id) DO NOTHING;
