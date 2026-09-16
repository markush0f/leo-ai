/**
 * Frontend transport boundary. Tauri commands reach native services; browser
 * preview uses an in-memory catalog. Keep command names and DTOs aligned with
 * `src-tauri/src/lib.rs` when extending this API.
 */
import { invoke } from "@tauri-apps/api/core";
import type { Conversation, Engine, Op, Snapshot, Turn } from "./types";

export const inTauri =
  typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

const GROK = "00000000-0000-4000-8000-000000000001";
const GPT = "00000000-0000-4000-8000-000000000002";
const MID = "00000000-0000-4000-8000-000000000101";
const STT = "00000000-0000-4000-8000-000000000501";
const TTS = "00000000-0000-4000-8000-000000000601";
const WAKE = "00000000-0000-4000-8000-000000000701";

const mockEngines: Engine[] = [
  { id: STT, role: "stt", kind: "grok", name: "grok-stt" },
  { id: "00000000-0000-4000-8000-000000000502", role: "stt", kind: "null", name: "none" },
  { id: TTS, role: "tts", kind: "null", name: "tone" },
  { id: WAKE, role: "wake", kind: "noop", name: "none" },
];

function mockSnap(): Snapshot {
  return {
    providers: [
      {
        id: GROK,
        name: "grok",
        kind: "grok",
        base_url: "https://api.x.ai/v1",
        key: "env",
      },
      {
        id: GPT,
        name: "gpt",
        kind: "gpt",
        base_url: "https://api.openai.com/v1",
        key: "falta",
      },
    ],
    models: [
      { id: MID, provider_id: GROK, name: "grok-4.6" },
      {
        id: "00000000-0000-4000-8000-000000000102",
        provider_id: GROK,
        name: "grok-4.5",
      },
      {
        id: "00000000-0000-4000-8000-000000000201",
        provider_id: GPT,
        name: "gpt-4.1",
      },
    ],
    engines: mockEngines,
    active_model_id: MID,
    active_conversation_id: null,
    system: "Eres Leo, un asistente. Responde en español, claro y directo.",
    voice_system:
      "Eres Leo, un asistente de voz. Responde en español, breve y claro, para ser leído en voz alta.",
    stt_engine_id: STT,
    tts_engine_id: TTS,
    wake_engine_id: WAKE,
    stt_language: "es",
    thinking: false,
    tools_enabled: true,
    tools: ["read_file", "execute_command", "get_weather", "list_processes"],
  };
}

let mock: Snapshot = mockSnap();
let mockChats: Conversation[] = [];
const mockTurns = new Map<string, Turn[]>();

/** Loads the catalog; preview callers receive a clone rather than shared mutable state. */
export async function loadSnapshot(): Promise<Snapshot> {
  if (inTauri) return invoke<Snapshot>("snapshot");
  return structuredClone(mock);
}

/** Applies a catalog mutation and returns the updated snapshot. */
export async function applyOp(op: Op): Promise<Snapshot> {
  if (inTauri) return invoke<Snapshot>("apply", { op });
  mock = applyMock(mock, op);
  return structuredClone(mock);
}

export async function listChats(): Promise<Conversation[]> {
  if (inTauri) return invoke<Conversation[]>("list_chats");
  return structuredClone(mockChats);
}

export async function openChat(id: string): Promise<Turn[]> {
  if (inTauri) return invoke<Turn[]>("open_chat", { id });
  mock.active_conversation_id = id;
  return structuredClone(mockTurns.get(id) ?? []);
}

export async function newChat(): Promise<Conversation> {
  if (inTauri) return invoke<Conversation>("new_chat");
  const conv: Conversation = { id: crypto.randomUUID(), title: null };
  mockChats = [conv, ...mockChats];
  mockTurns.set(conv.id, []);
  mock.active_conversation_id = conv.id;
  return { ...conv };
}

export async function sendChat(conversationId: string, text: string): Promise<string> {
  if (inTauri) {
    const out = await invoke<{ text: string }>("chat", {
      conversationId,
      text,
    });
    return out.text;
  }
  const turns = mockTurns.get(conversationId) ?? [];
  turns.push({ id: crypto.randomUUID(), role: "user", content: text });
  await new Promise((r) => setTimeout(r, 400));
  const reply =
    "Estoy en la vista previa del navegador. Para hablar de verdad: npm run tauri dev.";
  turns.push({ id: crypto.randomUUID(), role: "assistant", content: reply });
  mockTurns.set(conversationId, turns);
  const chat = mockChats.find((c) => c.id === conversationId);
  if (chat && !chat.title) chat.title = text.slice(0, 60);
  return reply;
}

function applyMock(snap: Snapshot, op: Op): Snapshot {
  const next = structuredClone(snap);
  switch (op.op) {
    case "activate_model":
      next.active_model_id = op.id;
      break;
    case "activate_provider": {
      const model = next.models.find((m) => m.provider_id === op.id);
      next.active_model_id = model?.id ?? null;
      break;
    }
    case "set_kind": {
      const p = next.providers.find((x) => x.id === op.id);
      if (p) p.kind = op.kind;
      break;
    }
    case "set_system":
      next.system = op.text;
      break;
    case "set_voice_system":
      next.voice_system = op.text;
      break;
    case "set_api_key": {
      const p = next.providers.find((x) => x.id === op.id);
      if (p) p.key = op.api_key.trim() ? "db" : "falta";
      break;
    }
    case "set_base_url": {
      const p = next.providers.find((x) => x.id === op.id);
      if (p) p.base_url = op.base_url || null;
      break;
    }
    case "new_provider": {
      const id = crypto.randomUUID();
      next.providers.push({
        id,
        name: op.name,
        kind: "grok",
        base_url: null,
        key: "falta",
      });
      break;
    }
    case "new_model":
      next.models.push({
        id: crypto.randomUUID(),
        provider_id: op.provider_id,
        name: op.name,
      });
      break;
    case "rename_provider": {
      const p = next.providers.find((x) => x.id === op.id);
      if (p) p.name = op.name;
      break;
    }
    case "rename_model": {
      const m = next.models.find((x) => x.id === op.id);
      if (m) m.name = op.name;
      break;
    }
    case "delete_provider":
      if (next.providers.length > 1) {
        next.providers = next.providers.filter((p) => p.id !== op.id);
        next.models = next.models.filter((m) => m.provider_id !== op.id);
      }
      break;
    case "delete_model":
      next.models = next.models.filter((m) => m.id !== op.id);
      break;
    case "set_engine":
      if (op.role === "stt") next.stt_engine_id = op.id;
      if (op.role === "tts") next.tts_engine_id = op.id;
      if (op.role === "wake") next.wake_engine_id = op.id;
      break;
    case "set_stt_language":
      next.stt_language = op.text;
      break;
    case "set_thinking":
      next.thinking = op.value;
      break;
    case "set_tools_enabled":
      next.tools_enabled = op.value;
      break;
  }
  return next;
}
