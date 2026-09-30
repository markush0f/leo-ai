/**
 * Frontend contracts mirrored by the Rust DTOs in `src-tauri/src/lib.rs`.
 * Keep field names and tagged operation variants synchronized with `ira-api`.
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

export const EFFORTS = ["low", "medium", "high", "xhigh"] as const;

export type Model = {
  id: string;
  provider_id: string;
  name: string;
  display_name: string;
  effort: string;
  effort_options: string[];
  reasoning: boolean;
  context_window: number | null;
  output_limit: number | null;
  release_date: string | null;
  last_updated: string | null;
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
  web_search_enabled: boolean;
  web_search_context_size: string;
  voice_system: string;
  stt_engine_id: string | null;
  tts_engine_id: string | null;
  wake_engine_id: string | null;
  stt_language: string;
  thinking: boolean;
  tools_enabled: boolean;
  tools_mutate: boolean;
  tools: string[];
};

export type Service = {
  id: string;
  name: string;
  running: boolean;
  healthy: boolean;
  detail: string;
  autostart?: boolean;
  host_port?: number | null;
  container_port?: number | null;
  via_gateway?: boolean;
  kind?: "service" | "mcp" | string;
  description?: string;
  peer?: string | null;
};

export type Services = {
  ok: boolean;
  services: Service[];
  gateway_port?: number;
  error?: string | null;
};

export type McpServer = {
  id: string;
  name: string;
  transport: "stdio" | "streamable_http" | "remote_bridge";
  url: string | null;
  command: string | null;
  args: string[];
  env: Record<string, string>;
  headers: Record<string, string>;
  enabled: boolean;
  editable: boolean;
};

export type McpInput = Omit<McpServer, "id" | "editable" | "url" | "command"> & {
  url?: string;
  command?: string;
};

export type DatabaseConnection = {
  id: string;
  name: string;
  host: string;
  port: number;
  database: string;
  username: string;
  ssl_mode: string;
  enabled: boolean;
  password_set: boolean;
  last_test_ok: boolean | null;
  last_test_error: string | null;
  last_tested_at: string | null;
};

export type DatabaseInput = {
  name: string;
  host: string;
  port: number;
  database: string;
  username: string;
  password?: string;
  ssl_mode: string;
  enabled: boolean;
};

export type DatabaseTest = {
  ok: boolean;
  read_only: boolean;
  detail: string;
};

export type CodexLogin = {
  id: string;
  verification_url: string;
  user_code: string;
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
  kind: "user" | "ira" | "error";
  text: string;
  mcps?: string[];
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
  | { op: "set_model_effort"; id: string; effort: string }
  | { op: "set_tools_enabled"; value: boolean }
  | { op: "set_tools_mutate"; value: boolean };
