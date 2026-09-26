CREATE TABLE IF NOT EXISTS host_services (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    required BOOLEAN NOT NULL DEFAULT FALSE,
    position INT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

INSERT INTO host_services (id, name, required, position) VALUES
    ('postgres', 'Postgres', TRUE, 0),
    ('toolbox', 'Toolbox', TRUE, 1),
    ('ira-realtime', 'Realtime', FALSE, 2),
    ('colibri', 'Colibrì', FALSE, 3),
    ('veritas-kanban', 'Veritas', FALSE, 4),
    ('veritas-mcp', 'Veritas MCP', FALSE, 5)
ON CONFLICT (id) DO NOTHING;
