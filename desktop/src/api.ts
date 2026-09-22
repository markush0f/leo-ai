/**
 * Frontend transport boundary. Tauri commands reach native services; the
 * browser talks to `ira-server` over HTTP (`/api`, same catalog and Ollama
 * path). Keep command names and DTOs aligned with `ira-api` when extending.
 */
import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import type {
  CodexLogin,
  Conversation,
  DatabaseConnection,
  DatabaseInput,
  DatabaseTest,
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

export async function loadServices(): Promise<Services> {
  if (inTauri) return invoke<Services>("services");
  return http<Services>("/api/services");
}

export async function startServices(): Promise<Services> {
  if (inTauri) return invoke<Services>("start_services");
  return http<Services>("/api/services", { method: "POST" });
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
