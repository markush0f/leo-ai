/**
 * Frontend contracts mirrored by the Rust DTOs in `src-tauri/src/lib.rs`.
 * Keep field names and tagged operation variants synchronized with `leo-api`.
 */
/** Credential availability only; never the actual provider secret. */
export type KeyStatus = "db" | "env" | "falta" | "none";

export type Provider = {
  id: string;
  name: string;
  kind: string;
  base_url: string | null;
  key: KeyStatus;
};

export type Model = {
  id: string;
  provider_id: string;
  name: string;
};

export type Engine = {
  id: string;
  role: "stt" | "tts" | "wake" | string;
  kind: string;
  name: string;
};

export type Snapshot = {
  providers: Provider[];
  models: Model[];
  engines: Engine[];
  active_model_id: string | null;
  active_conversation_id: string | null;
  system: string;
  voice_system: string;
  stt_engine_id: string | null;
  tts_engine_id: string | null;
  wake_engine_id: string | null;
  stt_language: string;
  thinking: boolean;
  tools_enabled: boolean;
  tools: string[];
};

export type Service = {
  id: string;
  name: string;
  running: boolean;
  healthy: boolean;
  detail: string;
};

export type Services = {
  ok: boolean;
  services: Service[];
  error?: string | null;
};

export type Conversation = {
  id: string;
  title: string | null;
};

export type Turn = {
  id: string;
  role: string;
  content: string;
};

/** Model-visible history, excluding display-only errors and bubble IDs. */
export type ChatTurn = {
  role: "user" | "assistant";
  content: string;
};

export type Bubble = {
  id: string;
  kind: "user" | "leo" | "error";
  text: string;
};

/** Catalog mutation serialized with the `op` discriminator expected by Tauri. */
export type Op =
  | { op: "activate_provider"; id: string }
  | { op: "activate_model"; id: string }
  | { op: "set_kind"; id: string; kind: string }
  | { op: "set_system"; text: string }
  | { op: "set_voice_system"; text: string }
  | { op: "set_api_key"; id: string; api_key: string }
  | { op: "set_base_url"; id: string; base_url: string }
  | { op: "new_provider"; name: string }
  | { op: "new_model"; provider_id: string; name: string }
  | { op: "rename_provider"; id: string; name: string }
  | { op: "rename_model"; id: string; name: string }
  | { op: "delete_provider"; id: string }
  | { op: "delete_model"; id: string }
  | { op: "set_engine"; role: string; id: string }
  | { op: "set_stt_language"; text: string }
  | { op: "set_thinking"; value: boolean }
  | { op: "set_tools_enabled"; value: boolean };
