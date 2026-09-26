export type Conversation = { id: string };

type Snapshot = {
  providers: { id: string; name: string }[];
  models: { id: string; provider_id: string; name: string }[];
  active_model_id: string | null;
  tools_enabled: boolean;
};

export class IraClient {
  constructor(private readonly base: string) {}

  async ensure(jid: string): Promise<Conversation> {
    return this.post<Conversation>(`/api/channels/whatsapp/${encodeURIComponent(jid)}`);
  }

  async reset(jid: string): Promise<Conversation> {
    return this.post<Conversation>(`/api/channels/whatsapp/${encodeURIComponent(jid)}/reset`);
  }

  async chat(id: string, text: string): Promise<string> {
    const out = await this.post<{ text?: unknown }>(`/api/chats/${id}/messages`, { text });
    if (typeof out.text !== "string" || !out.text.trim()) {
      throw new Error("ira-server no devolvió texto");
    }
    return out.text;
  }

  async status(): Promise<string> {
    const snap = await this.get<Snapshot>("/api/snapshot");
    const model = snap.models.find((item) => item.id === snap.active_model_id);
    const provider = snap.providers.find((item) => item.id === model?.provider_id);
    const who = [provider?.name, model?.name].filter(Boolean).join(" · ") || "sin modelo";
    return `${who}\n${snap.tools_enabled ? "tools on" : "tools off"}`;
  }

  private async get<T>(path: string): Promise<T> {
    return this.send(path);
  }

  private async post<T>(path: string, body?: unknown): Promise<T> {
    return this.send(path, {
      method: "POST",
      body: body === undefined ? undefined : JSON.stringify(body),
    });
  }

  private async send<T>(path: string, init?: RequestInit): Promise<T> {
    let response: Response;
    try {
      response = await fetch(`${this.base}${path}`, {
        ...init,
        headers: {
          Accept: "application/json",
          ...(init?.body ? { "Content-Type": "application/json" } : {}),
        },
      });
    } catch (error) {
      throw new Error(`no se pudo conectar a ira-server (${this.base}): ${String(error)}`);
    }
    const raw = await response.text();
    let data: unknown = null;
    if (raw) {
      try {
        data = JSON.parse(raw);
      } catch {
        throw new Error(`ira-server respondió sin JSON (${response.status})`);
      }
    }
    if (!response.ok) {
      const error =
        data &&
        typeof data === "object" &&
        "error" in data &&
        typeof (data as { error: unknown }).error === "string"
          ? (data as { error: string }).error
          : raw || response.statusText;
      throw new Error(error);
    }
    return data as T;
  }
}
