import { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import { z } from "zod";

const API_URL = process.env.API_URL || "http://localhost:3200";

async function api(method: string, path: string, body?: unknown) {
  const opts: RequestInit = {
    method,
    headers: { "content-type": "application/json", accept: "application/json" },
  };
  if (body !== undefined) opts.body = JSON.stringify(body);
  const res = await fetch(`${API_URL}${path}`, opts);
  if (res.status === 204) return null;
  if (!res.ok) {
    const err = await res.text();
    throw new Error(`API ${res.status}: ${err}`);
  }
  return res.json();
}

const server = new McpServer({
  name: "projects-api",
  version: "1.0.0",
});

server.tool(
  "list_projects",
  "List all projects with task counts",
  {},
  async () => {
    const data = await api("GET", "/projects");
    return { content: [{ type: "text", text: JSON.stringify(data, null, 2) }] };
  }
);

server.tool(
  "get_project",
  "Get a project by ID",
  { id: z.string().describe("Project ID") },
  async ({ id }) => {
    const data = await api("GET", `/projects/${id}`);
    return { content: [{ type: "text", text: JSON.stringify(data, null, 2) }] };
  }
);

server.tool(
  "create_project",
  "Create a new project",
  {
    name: z.string().describe("Project name"),
    description: z.string().optional().describe("Project description"),
  },
  async ({ name, description }) => {
    const data = await api("POST", "/projects", { name, description });
    return { content: [{ type: "text", text: JSON.stringify(data, null, 2) }] };
  }
);

server.tool(
  "update_project",
  "Update a project",
  {
    id: z.string().describe("Project ID"),
    name: z.string().optional().describe("New name"),
    description: z.string().optional().describe("New description"),
  },
  async ({ id, name, description }) => {
    const data = await api("PUT", `/projects/${id}`, { name, description });
    return { content: [{ type: "text", text: JSON.stringify(data, null, 2) }] };
  }
);

server.tool(
  "delete_project",
  "Delete a project and all its tasks",
  { id: z.string().describe("Project ID") },
  async ({ id }) => {
    await api("DELETE", `/projects/${id}`);
    return { content: [{ type: "text", text: "Project deleted" }] };
  }
);

server.tool(
  "list_tasks",
  "List all tasks in a project",
  { project_id: z.string().describe("Project ID") },
  async ({ project_id }) => {
    const data = await api("GET", `/projects/${project_id}/tasks`);
    return { content: [{ type: "text", text: JSON.stringify(data, null, 2) }] };
  }
);

server.tool(
  "get_task",
  "Get a task by ID",
  {
    project_id: z.string().describe("Project ID"),
    task_id: z.string().describe("Task ID"),
  },
  async ({ project_id, task_id }) => {
    const data = await api("GET", `/projects/${project_id}/tasks/${task_id}`);
    return { content: [{ type: "text", text: JSON.stringify(data, null, 2) }] };
  }
);

server.tool(
  "create_task",
  "Create a task in a project",
  {
    project_id: z.string().describe("Project ID"),
    title: z.string().describe("Task title"),
    description: z.string().optional().describe("Task description"),
    status: z.enum(["todo", "in_progress", "done"]).optional().describe("Task status"),
    priority: z.enum(["low", "medium", "high"]).optional().describe("Task priority"),
  },
  async ({ project_id, title, description, status, priority }) => {
    const data = await api("POST", `/projects/${project_id}/tasks`, {
      title, description, status, priority,
    });
    return { content: [{ type: "text", text: JSON.stringify(data, null, 2) }] };
  }
);

server.tool(
  "update_task",
  "Update a task",
  {
    project_id: z.string().describe("Project ID"),
    task_id: z.string().describe("Task ID"),
    title: z.string().optional().describe("New title"),
    description: z.string().optional().describe("New description"),
    status: z.enum(["todo", "in_progress", "done"]).optional().describe("New status"),
    priority: z.enum(["low", "medium", "high"]).optional().describe("New priority"),
  },
  async ({ project_id, task_id, title, description, status, priority }) => {
    const data = await api("PUT", `/projects/${project_id}/tasks/${task_id}`, {
      title, description, status, priority,
    });
    return { content: [{ type: "text", text: JSON.stringify(data, null, 2) }] };
  }
);

server.tool(
  "delete_task",
  "Delete a task",
  {
    project_id: z.string().describe("Project ID"),
    task_id: z.string().describe("Task ID"),
  },
  async ({ project_id, task_id }) => {
    await api("DELETE", `/projects/${project_id}/tasks/${task_id}`);
    return { content: [{ type: "text", text: "Task deleted" }] };
  }
);

async function main() {
  const transport = new StdioServerTransport();
  await server.connect(transport);
}

main().catch((err) => {
  console.error("MCP server error:", err);
  process.exit(1);
});
