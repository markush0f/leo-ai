/**
 * Frontend transport boundary. Tauri commands reach native services; the
 * browser talks to `leo-server` over HTTP (`/api`, same catalog and Ollama
 * path). Keep command names and DTOs aligned with `leo-api` when extending.
 */
import { invoke } from "@tauri-apps/api/core";
import type { Conversation, Op, Services, Snapshot, Turn } from "./types";

export const inTauri =
  typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

const apiBase = (import.meta.env.VITE_LEO_API as string | undefined)?.replace(/\/$/, "") ?? "";

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
    throw new Error("no se pudo conectar a leo-server. arráncalo: cargo run -p leo-server");
  }
  const raw = await res.text();
  let data: unknown = null;
  if (raw) {
    try {
      data = JSON.parse(raw);
    } catch {
      throw new Error("no se pudo conectar a leo-server. arráncalo: cargo run -p leo-server");
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

/** Loads the catalog from Tauri or `leo-server`. */
export async function loadSnapshot(): Promise<Snapshot> {
  if (inTauri) return invoke<Snapshot>("snapshot");
  return http<Snapshot>("/api/snapshot");
}

/** Applies a catalog mutation and returns the updated snapshot. */
export async function applyOp(op: Op): Promise<Snapshot> {
  if (inTauri) return invoke<Snapshot>("apply", { op });
  return http<Snapshot>("/api/apply", { method: "POST", body: JSON.stringify(op) });
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
