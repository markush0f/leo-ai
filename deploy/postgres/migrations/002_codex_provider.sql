INSERT INTO providers (id, name, kind, base_url) VALUES
    ('00000000-0000-4000-8000-000000000005', 'codex', 'codex', 'https://chatgpt.com/backend-api/codex')
ON CONFLICT (id) DO UPDATE SET
    kind = EXCLUDED.kind,
    base_url = EXCLUDED.base_url;

INSERT INTO models (id, provider_id, name) VALUES
    ('00000000-0000-4000-8000-000000000501', '00000000-0000-4000-8000-000000000005', 'gpt-5.4')
ON CONFLICT (id) DO NOTHING;
