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

export type Snapshot = {
  providers: Provider[];
  models: Model[];
  active_model_id: string | null;
  system: string;
  tools: string[];
};

export type Voice = {
  running: boolean;
  ok: boolean;
  state: string;
  message: string | null;
};

export type ChatTurn = {
  role: "user" | "assistant";
  content: string;
};

export type Bubble = {
  id: string;
  kind: "user" | "leo" | "error";
  text: string;
};

export type Op =
  | { op: "activate_provider"; id: string }
  | { op: "activate_model"; id: string }
  | { op: "set_kind"; id: string; kind: string }
  | { op: "set_system"; text: string }
  | { op: "set_api_key"; id: string; api_key: string }
  | { op: "set_base_url"; id: string; base_url: string }
  | { op: "new_provider"; name: string }
  | { op: "new_model"; provider_id: string; name: string }
  | { op: "rename_provider"; id: string; name: string }
  | { op: "rename_model"; id: string; name: string }
  | { op: "delete_provider"; id: string }
  | { op: "delete_model"; id: string };
