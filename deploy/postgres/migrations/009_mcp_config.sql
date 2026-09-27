ALTER TABLE mcp_manager ADD COLUMN IF NOT EXISTS name TEXT;
UPDATE mcp_manager SET name = id WHERE name IS NULL;
ALTER TABLE mcp_manager ALTER COLUMN name SET NOT NULL;
ALTER TABLE mcp_manager ADD COLUMN IF NOT EXISTS transport TEXT NOT NULL DEFAULT 'streamable_http';
ALTER TABLE mcp_manager ALTER COLUMN url DROP NOT NULL;
ALTER TABLE mcp_manager ADD COLUMN IF NOT EXISTS command TEXT;
ALTER TABLE mcp_manager ADD COLUMN IF NOT EXISTS args JSONB NOT NULL DEFAULT '[]'::jsonb;
ALTER TABLE mcp_manager ADD COLUMN IF NOT EXISTS env JSONB NOT NULL DEFAULT '{}'::jsonb;
ALTER TABLE mcp_manager DROP CONSTRAINT IF EXISTS mcp_manager_transport_check;
ALTER TABLE mcp_manager ADD CONSTRAINT mcp_manager_transport_check CHECK (transport IN ('stdio', 'streamable_http'));
ALTER TABLE mcp_manager DROP CONSTRAINT IF EXISTS mcp_manager_shape_check;
ALTER TABLE mcp_manager ADD CONSTRAINT mcp_manager_shape_check CHECK (
    (transport = 'stdio' AND command IS NOT NULL AND url IS NULL)
    OR (transport = 'streamable_http' AND url IS NOT NULL AND command IS NULL)
);
