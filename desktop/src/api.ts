import { invoke } from "@tauri-apps/api/core";
import type { ChatTurn, Op, Snapshot, Voice } from "./types";

export const inTauri =
  typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

const GROK = "00000000-0000-4000-8000-000000000001";
const GPT = "00000000-0000-4000-8000-000000000002";
const MID = "00000000-0000-4000-8000-000000000101";

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
    active_model_id: MID,
    system: "Eres Leo, un asistente. Responde en español, claro y directo.",
    tools: ["read_file", "execute_command", "get_weather", "list_processes"],
  };
}

let mock: Snapshot = mockSnap();

export async function loadSnapshot(): Promise<Snapshot> {
  if (inTauri) return invoke<Snapshot>("snapshot");
  return structuredClone(mock);
}

export async function applyOp(op: Op): Promise<Snapshot> {
  if (inTauri) return invoke<Snapshot>("apply", { op });
  mock = applyMock(mock, op);
  return structuredClone(mock);
}

export type ChatOpts = {
  thinking?: boolean;
  tools?: boolean;
};

export async function sendChat(
  messages: ChatTurn[],
  opts: ChatOpts = {},
): Promise<string> {
  if (inTauri) {
    const out = await invoke<{ text: string }>("chat", {
      messages,
      thinking: opts.thinking ?? false,
      tools: opts.tools ?? true,
    });
    return out.text;
  }
  await new Promise((r) => setTimeout(r, 500));
  return "Estoy en la vista previa del navegador. Para hablar de verdad: npm run tauri dev.";
}

export async function voiceStatus(): Promise<Voice> {
  if (inTauri) return invoke<Voice>("voice_status");
  return {
    running: false,
    ok: false,
    state: "apagado",
    message: "el daemon no está en marcha",
  };
}

export async function voiceListen(): Promise<Voice> {
  if (inTauri) return invoke<Voice>("voice_listen");
  return voiceStatus();
}

export async function voiceStop(): Promise<Voice> {
  if (inTauri) return invoke<Voice>("voice_stop");
  return voiceStatus();
}

export async function voiceShutdown(): Promise<Voice> {
  if (inTauri) return invoke<Voice>("voice_shutdown");
  return voiceStatus();
}

export async function voiceSpeak(text: string): Promise<Voice> {
  if (inTauri) return invoke<Voice>("voice_speak", { text });
  return voiceStatus();
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
  }
  return next;
}
