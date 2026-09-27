DELETE FROM conversations WHERE channel = 'whatsapp';

DROP INDEX IF EXISTS conversations_whatsapp_live;

ALTER TABLE conversations DROP CONSTRAINT IF EXISTS conversations_channel_check;

ALTER TABLE conversations
    ADD CONSTRAINT conversations_channel_check
    CHECK (channel IN ('local', 'telegram', 'voice'));
