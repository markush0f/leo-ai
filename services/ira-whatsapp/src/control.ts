import { createServer, type IncomingMessage, type ServerResponse } from "node:http";

export type WaStatus = {
  ok: true;
  phase: "connecting" | "qr" | "open" | "closed";
  user: string | null;
  qr: string | null;
  allow_phones: string[];
  auth_dir: string;
};

export type WaControl = {
  status: () => WaStatus;
  pair: () => Promise<void>;
  setAllow: (raw: string) => Promise<void>;
};

export function listenControl(port: number, bridge: WaControl): Promise<void> {
  const server = createServer((req, res) => {
    void handle(req, res, bridge);
  });
  return new Promise((resolve, reject) => {
    server.once("error", reject);
    server.listen(port, "127.0.0.1", () => {
      console.log(`control WhatsApp en 127.0.0.1:${port}`);
      resolve();
    });
  });
}

async function handle(req: IncomingMessage, res: ServerResponse, bridge: WaControl): Promise<void> {
  const path = new URL(req.url || "/", "http://127.0.0.1").pathname;
  try {
    if (req.method === "GET" && path === "/status") {
      return send(res, 200, bridge.status());
    }
    if (req.method === "POST" && path === "/pair") {
      await bridge.pair();
      return send(res, 200, bridge.status());
    }
    if (req.method === "PUT" && path === "/allow") {
      const body = await readBody(req);
      const phones = typeof body.phones === "string" ? body.phones : "";
      await bridge.setAllow(phones);
      return send(res, 200, bridge.status());
    }
    send(res, 404, { error: "no encontrado" });
  } catch (error) {
    send(res, 500, { error: error instanceof Error ? error.message : String(error) });
  }
}

function readBody(req: IncomingMessage): Promise<{ phones?: unknown }> {
  return new Promise((resolve, reject) => {
    const chunks: Buffer[] = [];
    req.on("data", (chunk: Buffer) => chunks.push(chunk));
    req.on("end", () => {
      const raw = Buffer.concat(chunks).toString("utf8").trim();
      if (!raw) {
        resolve({});
        return;
      }
      try {
        const parsed = JSON.parse(raw) as { phones?: unknown };
        resolve(parsed);
      } catch {
        reject(new Error("JSON inválido"));
      }
    });
    req.on("error", reject);
  });
}

function send(res: ServerResponse, status: number, body: unknown): void {
  const raw = JSON.stringify(body);
  res.writeHead(status, {
    "Content-Type": "application/json",
    "Content-Length": Buffer.byteLength(raw),
  });
  res.end(raw);
}
