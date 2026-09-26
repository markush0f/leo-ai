ALTER TABLE models ADD COLUMN IF NOT EXISTS effort TEXT NOT NULL DEFAULT 'low';

UPDATE models
SET effort = 'high'
WHERE effort = 'low'
  AND EXISTS (SELECT 1 FROM settings WHERE id = 1 AND thinking);
