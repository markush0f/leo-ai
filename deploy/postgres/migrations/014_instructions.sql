CREATE TABLE IF NOT EXISTS instructions (
    key TEXT NOT NULL,
    channel TEXT NOT NULL,
    content TEXT NOT NULL,
    active BOOLEAN NOT NULL DEFAULT TRUE,
    position INT NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (key, channel),
    CHECK (channel IN ('all', 'chat', 'voice')),
    CHECK (key ~ '^[a-z][a-z0-9_]{0,31}$'),
    CHECK (char_length(content) BETWEEN 1 AND 8000)
);

INSERT INTO instructions (key, channel, content, position)
SELECT 'persona', 'chat',
       CASE
           WHEN btrim(system_prompt) = '' THEN 'Eres Ira, un asistente. Responde en español, claro y directo.'
           ELSE left(system_prompt, 8000)
       END,
       10
FROM settings
WHERE id = 1
ON CONFLICT (key, channel) DO NOTHING;

INSERT INTO instructions (key, channel, content, position)
SELECT 'persona', 'voice',
       CASE
           WHEN btrim(voice_system_prompt) = '' THEN 'Eres Ira, un asistente de voz. Responde en español, breve y claro, para ser leído en voz alta.'
           ELSE left(voice_system_prompt, 8000)
       END,
       10
FROM settings
WHERE id = 1
ON CONFLICT (key, channel) DO NOTHING
