UPDATE settings
SET system_prompt = 'Eres Ira, un asistente. Responde en español, claro y directo.'
WHERE system_prompt = 'Eres Leo, un asistente. Responde en español, claro y directo.';

UPDATE settings
SET voice_system_prompt = 'Eres Ira, un asistente de voz. Responde en español, breve y claro, para ser leído en voz alta.'
WHERE voice_system_prompt = 'Eres Leo, un asistente de voz. Responde en español, breve y claro, para ser leído en voz alta.';

ALTER TABLE settings ALTER COLUMN voice_system_prompt
    SET DEFAULT 'Eres Ira, un asistente de voz. Responde en español, breve y claro, para ser leído en voz alta.';
