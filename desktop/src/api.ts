/**
 * Frontend transport boundary. Tauri commands reach native services; the
 * browser talks to `ira-server` over HTTP (`/api`, same catalog and Ollama
 * path). Keep command names and DTOs aligned with `ira-api` when extending.
 */
import { Channel, invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import type {
  CodexLogin,
  Conversation,
  DatabaseConnection,
  DatabaseInput,
  DatabaseTest,
  McpInput,
  McpServer,
  Op,
  Services,
  Snapshot,
  Turn,
} from "./types";

export const inTauri =
  typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

const apiBase = (import.meta.env.VITE_IRA_API as string | undefined)?.replace(/\/$/, "") ?? "";

function url(path: string): string {
  return `${apiBase}${path}`;
}

async function http<T>(path: string, init?: RequestInit): Promise<T> {
  let res: Response;
  try {
    res = await fetch(url(path), {
      ...init,
      headers: {
        Accept: "application/json",
        ...(init?.body ? { "Content-Type": "application/json" } : {}),
        ...(init?.headers ?? {}),
      },
    });
  } catch {
    throw new Error("no se pudo conectar a ira-server. arráncalo: cargo run -p ira-server");
  }
  const raw = await res.text();
  let data: unknown = null;
  if (raw) {
    try {
      data = JSON.parse(raw);
    } catch {
      throw new Error("no se pudo conectar a ira-server. arráncalo: cargo run -p ira-server");
    }
  }
  if (!res.ok) {
    const err =
      data &&
      typeof data === "object" &&
      "error" in data &&
      typeof (data as { error: unknown }).error === "string"
        ? (data as { error: string }).error
        : raw || res.statusText;
    throw new Error(err);
  }
  return data as T;
}

/** Loads the catalog from Tauri or `ira-server`. */
export async function loadSnapshot(): Promise<Snapshot> {
  if (inTauri) return invoke<Snapshot>("snapshot");
  return http<Snapshot>("/api/snapshot");
}

/** Applies a catalog mutation and returns the updated snapshot. */
export async function applyOp(op: Op): Promise<Snapshot> {
  if (inTauri) return invoke<Snapshot>("apply", { op });
  return http<Snapshot>("/api/apply", { method: "POST", body: JSON.stringify(op) });
}

export async function updateWebSearch(enabled: boolean, contextSize: string): Promise<{
  web_search_enabled: boolean;
  web_search_context_size: string;
}> {
  if (inTauri) {
    return invoke<{ web_search_enabled: boolean; web_search_context_size: string }>(
      "update_web_search",
      { enabled, contextSize },
    );
  }
  return http<{ web_search_enabled: boolean; web_search_context_size: string }>(
    "/api/preferences/web-search",
    {
    method: "PUT",
    body: JSON.stringify({ enabled, context_size: contextSize }),
    },
  );
}

export async function beginCodexLogin(providerId: string): Promise<CodexLogin> {
  if (inTauri) return invoke<CodexLogin>("begin_codex_login", { providerId });
  return http<CodexLogin>("/api/codex/login", {
    method: "POST",
    body: JSON.stringify({ provider_id: providerId }),
  });
}

export async function finishCodexLogin(id: string): Promise<Snapshot> {
  if (inTauri) return invoke<Snapshot>("finish_codex_login", { id });
  return http<Snapshot>(`/api/codex/login/${id}/finish`, { method: "POST" });
}

export async function openExternal(url: string): Promise<void> {
  if (inTauri) {
    await openUrl(url);
    return;
  }
  window.open(url, "_blank", "noopener,noreferrer");
}

export async function listChats(): Promise<Conversation[]> {
  if (inTauri) return invoke<Conversation[]>("list_chats");
  return http<Conversation[]>("/api/chats");
}

export async function openChat(id: string): Promise<Turn[]> {
  if (inTauri) return invoke<Turn[]>("open_chat", { id });
  return http<Turn[]>(`/api/chats/${id}`);
}

export async function newChat(): Promise<Conversation> {
  if (inTauri) return invoke<Conversation>("new_chat");
  return http<Conversation>("/api/chats", { method: "POST" });
}

export async function renameChat(id: string, title: string): Promise<Conversation[]> {
  if (inTauri) return invoke<Conversation[]>("rename_chat", { id, title });
  return http<Conversation[]>(`/api/chats/${id}`, { method: "PATCH", body: JSON.stringify({ title }) });
}

export async function deleteChat(id: string): Promise<Conversation[]> {
  if (inTauri) return invoke<Conversation[]>("delete_chat", { id });
  return http<Conversation[]>(`/api/chats/${id}`, { method: "DELETE" });
}

export async function deleteAllChats(): Promise<Conversation[]> {
  if (inTauri) return invoke<Conversation[]>("delete_all_chats");
  return http<Conversation[]>("/api/chats", { method: "DELETE" });
}

export async function loadServices(): Promise<Services> {
  if (inTauri) return invoke<Services>("services");
  return http<Services>("/api/services");
}

export async function listMcp(): Promise<McpServer[]> {
  if (inTauri) return invoke<McpServer[]>("list_mcp");
  return http<McpServer[]>("/api/mcp");
}

export async function saveMcp(input: McpInput): Promise<McpServer> {
  if (inTauri) return invoke<McpServer>("save_mcp", { input });
  return http<McpServer>("/api/mcp", { method: "POST", body: JSON.stringify(input) });
}

export async function deleteMcp(id: string): Promise<void> {
  if (inTauri) await invoke("delete_mcp", { id });
  else await http(`/api/mcp/${encodeURIComponent(id)}`, { method: "DELETE" });
}

export async function testMcp(id: string): Promise<{ tools: string[] }> {
  if (inTauri) return invoke<{ tools: string[] }>("test_mcp", { id });
  return http<{ tools: string[] }>(`/api/mcp/${encodeURIComponent(id)}/test`, { method: "POST" });
}

export async function startServices(): Promise<Services> {
  if (inTauri) return invoke<Services>("start_services");
  return http<Services>("/api/services", { method: "POST" });
}

export async function setService(
  id: string,
  action: "start" | "stop" | "autostart" | "manual" | "port" | "meta",
  port?: number,
  name?: string,
  description?: string,
): Promise<Services> {
  if (inTauri) {
    return invoke<Services>("set_service", { id, action, port: port ?? null, name: name ?? null, description: description ?? null });
  }
  return http<Services>(`/api/services/${encodeURIComponent(id)}`, {
    method: "POST",
    body: JSON.stringify({ action, port, name, description }),
  });
}

export async function sendChat(conversationId: string, text: string): Promise<string> {
  if (inTauri) {
    const out = await invoke<{ text: string }>("chat", {
      conversationId,
      text,
    });
    return out.text;
  }
  const out = await http<{ text: string }>(`/api/chats/${conversationId}/messages`, {
    method: "POST",
    body: JSON.stringify({ text }),
  });
  return out.text;
}

export type ChatStreamEvent =
  | { type: "delta"; text: string }
  | { type: "reset" }
  | { type: "mcp_used"; server_id: string; tool_name: string }
  | { type: "done" }
  | { type: "error"; error: string };

export async function streamChat(
  conversationId: string,
  text: string,
  onEvent: (event: ChatStreamEvent) => void,
): Promise<void> {
  let streamError: string | null = null;
  const receive = (event: ChatStreamEvent) => {
    if (event.type === "error") streamError = event.error;
    onEvent(event);
  };

  if (inTauri) {
    const sink = new Channel<ChatStreamEvent>();
    sink.onmessage = receive;
    try {
      await invoke<void>("chat_stream", { conversationId, text, sink });
    } catch (error) {
      if (!streamError) throw error;
    }
    if (streamError) throw new Error(streamError);
    return;
  }

  let response: Response;
  try {
    response = await fetch(url(`/api/chats/${conversationId}/messages/stream`), {
      method: "POST",
      headers: {
        Accept: "application/x-ndjson",
        "Content-Type": "application/json",
      },
      body: JSON.stringify({ text }),
    });
  } catch {
    throw new Error("no se pudo conectar a ira-server. arráncalo: cargo run -p ira-server");
  }

  if (!response.ok) {
    const raw = await response.text();
    try {
      const parsed = JSON.parse(raw) as { error?: string };
      throw new Error(parsed.error || raw || response.statusText);
    } catch (error) {
      if (error instanceof SyntaxError) throw new Error(raw || response.statusText);
      throw error;
    }
  }
  if (!response.body) throw new Error("ira-server no devolvió un stream");

  const reader = response.body.getReader();
  const decoder = new TextDecoder();
  let buffer = "";
  let done = false;
  const consume = (line: string) => {
    const trimmed = line.trim();
    if (!trimmed) return;
    const event = JSON.parse(trimmed) as ChatStreamEvent;
    if (event.type === "done") done = true;
    receive(event);
  };

  while (true) {
    const chunk = await reader.read();
    buffer += decoder.decode(chunk.value, { stream: !chunk.done });
    const lines = buffer.split("\n");
    buffer = lines.pop() ?? "";
    lines.forEach(consume);
    if (chunk.done) break;
  }
  consume(buffer);
  if (streamError) throw new Error(streamError);
  if (!done) throw new Error("el stream terminó antes de completar la respuesta");
}

export async function listDatabases(): Promise<DatabaseConnection[]> {
  if (inTauri) return invoke<DatabaseConnection[]>("list_databases");
  return http<DatabaseConnection[]>("/api/databases");
}

export async function createDatabase(input: DatabaseInput): Promise<DatabaseConnection> {
  if (inTauri) return invoke<DatabaseConnection>("create_database", { input });
  return http<DatabaseConnection>("/api/databases", {
    method: "POST",
    body: JSON.stringify(input),
  });
}

export async function updateDatabase(id: string, input: DatabaseInput): Promise<DatabaseConnection> {
  if (inTauri) return invoke<DatabaseConnection>("update_database", { id, input });
  return http<DatabaseConnection>(`/api/databases/${id}`, {
    method: "PUT",
    body: JSON.stringify(input),
  });
}

export async function deleteDatabase(id: string): Promise<{ ok: boolean }> {
  if (inTauri) return invoke<{ ok: boolean }>("delete_database", { id });
  return http<{ ok: boolean }>(`/api/databases/${id}`, { method: "DELETE" });
}

export async function testDatabase(id: string): Promise<DatabaseTest> {
  if (inTauri) return invoke<DatabaseTest>("test_database", { id });
  return http<DatabaseTest>(`/api/databases/${id}/test`, { method: "POST" });
}
