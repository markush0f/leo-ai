ALTER TABLE conversations DROP CONSTRAINT IF EXISTS conversations_channel_check;

ALTER TABLE conversations
    ADD CONSTRAINT conversations_channel_check
    CHECK (channel IN ('local', 'telegram', 'voice', 'whatsapp'));

CREATE UNIQUE INDEX IF NOT EXISTS conversations_whatsapp_live
    ON conversations (external_id)
    WHERE channel = 'whatsapp' AND archived_at IS NULL AND external_id IS NOT NULL;
