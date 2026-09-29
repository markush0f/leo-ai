DELETE FROM host_services WHERE id IN ('veritas-kanban', 'veritas-mcp');

DELETE FROM mcp_manager
WHERE id IN ('veritas', 'veritas-kanban', 'veritas-mcp')
   OR name ILIKE '%veritas%';
