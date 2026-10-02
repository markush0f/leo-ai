CREATE TABLE IF NOT EXISTS memories (
    id UUID PRIMARY KEY,
    content TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK (length(btrim(content)) BETWEEN 1 AND 8000)
);

CREATE INDEX IF NOT EXISTS memories_search
    ON memories USING GIN (to_tsvector('simple', content));
