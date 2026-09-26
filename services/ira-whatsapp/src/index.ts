import { chmod, mkdir, readdir, rm } from "node:fs/promises";
import path from "node:path";
import QRCode from "qrcode";
import makeWASocket, {
  Browsers,
  DisconnectReason,
  fetchLatestBaileysVersion,
  makeCacheableSignalKeyStore,
  useMultiFileAuthState,
  type WAMessage,
  type WASocket,
} from "@whiskeysockets/baileys";
import pino from "pino";
import qrcode from "qrcode-terminal";
import { mayReply, parseAllowPhones } from "./allow.ts";
import { loadAllow, saveAllow } from "./allow-store.ts";
import { HELP, route } from "./commands.ts";
import { listenControl, type WaStatus } from "./control.ts";
import { loadConfig, type Config } from "./config.ts";
import { IraClient } from "./ira.ts";
import { splitWhatsApp } from "./split.ts";
import { messageText } from "./text.ts";

const logger = pino({ level: process.env.WHATSAPP_LOG ?? "warn" });

async function tighten(dir: string): Promise<void> {
  await chmod(dir, 0o700);
  const names = await readdir(dir);
  await Promise.all(
    names.map((name) => chmod(path.join(dir, name), 0o600).catch(() => undefined)),
  );
}

function disconnectCode(error: unknown): number | undefined {
  if (!error || typeof error !== "object" || !("output" in error)) return undefined;
  const output = (error as { output?: { statusCode?: unknown } }).output;
  return typeof output?.statusCode === "number" ? output.statusCode : undefined;
}

type Tail = Promise<void>;

function enqueue(tails: Map<string, Tail>, jid: string, job: () => Promise<void>): void {
  const prev = tails.get(jid) ?? Promise.resolve();
  const next = prev.then(job, job);
  tails.set(
    jid,
    next.then(
      () => undefined,
      () => undefined,
    ),
  );
}

function ownJids(sock: WASocket): string[] {
  const creds = sock.authState.creds.me;
  return [sock.user?.id, sock.user?.lid, creds?.id, creds?.lid].filter(
    (jid): jid is string => Boolean(jid),
  );
}

function remember(set: Set<string>, id: string, cap = 500): void {
  set.add(id);
  while (set.size > cap) {
    const first = set.values().next().value;
    if (!first) break;
    set.delete(first);
  }
}

async function dispatch(ira: IraClient, jid: string, text: string): Promise<string> {
  const command = route(text);
  if (command.type === "help") return HELP;
  if (command.type === "unknown") return `comando desconocido: /${command.name}\n${HELP}`;
  if (command.type === "status") return ira.status();
  if (command.type === "clear") {
    await ira.reset(jid);
    return "conversación nueva";
  }
  const conversation = await ira.ensure(jid);
  return ira.chat(conversation.id, command.text);
}

async function reply(
  sock: WASocket,
  jid: string,
  text: string,
  ownSends: Set<string>,
  outbound: Set<string>,
): Promise<void> {
  for (const chunk of splitWhatsApp(text)) {
    remember(outbound, chunk);
    const sent = await sock.sendMessage(jid, { text: chunk });
    if (sent?.key.id) remember(ownSends, sent.key.id);
  }
}

type Live = {
  allowPhones: string[];
};

async function onMessage(
  sock: WASocket,
  ira: IraClient,
  live: Live,
  ownSends: Set<string>,
  outbound: Set<string>,
  seen: Set<string>,
  msg: WAMessage,
): Promise<void> {
  const jid = msg.key.remoteJid;
  const id = msg.key.id;
  if (!jid || !id) return;
  if (ownSends.has(id)) return;
  if (seen.has(id)) return;
  remember(seen, id);
  if (
    !mayReply({
      remoteJid: jid,
      fromMe: Boolean(msg.key.fromMe),
      ownJids: ownJids(sock),
      allowPhones: live.allowPhones,
    })
  ) {
    return;
  }
  const text = messageText(msg.message);
  if (!text) return;
  if (msg.key.fromMe && outbound.has(text)) return;

  let answer: string;
  try {
    await sock.sendPresenceUpdate("composing", jid);
    answer = await dispatch(ira, jid, text);
  } catch (error) {
    answer = error instanceof Error ? error.message : String(error);
  } finally {
    await sock.sendPresenceUpdate("paused", jid).catch(() => undefined);
  }
  await reply(sock, jid, answer, ownSends, outbound);
}

async function clearSession(dir: string): Promise<void> {
  const names = await readdir(dir);
  await Promise.all(
    names
      .filter((name) => name !== "allow.json")
      .map((name) => rm(path.join(dir, name), { recursive: true, force: true })),
  );
}

type Session = {
  phase: WaStatus["phase"];
  user: string | null;
  qr: string | null;
  pairing: boolean;
  sock: WASocket | null;
};

async function connect(cfg: Config, ira: IraClient, live: Live, session: Session): Promise<void> {
  const { state, saveCreds } = await useMultiFileAuthState(cfg.authDir);
  let version: [number, number, number] | undefined;
  try {
    version = (await fetchLatestBaileysVersion()).version;
  } catch (error) {
    logger.warn({ err: error }, "no pude leer la versión de WhatsApp; uso la de Baileys");
  }
  const sock = makeWASocket({
    version,
    auth: {
      creds: state.creds,
      keys: makeCacheableSignalKeyStore(state.keys, logger),
    },
    logger,
    browser: Browsers.ubuntu("Leo"),
    markOnlineOnConnect: false,
  });
  session.sock = sock;
  session.phase = "connecting";
  sock.ev.on("creds.update", async () => {
    await saveCreds();
    await tighten(cfg.authDir);
  });

  const ownSends = new Set<string>();
  const outbound = new Set<string>();
  const seen = new Set<string>();
  const tails = new Map<string, Tail>();
  sock.ev.on("messages.upsert", ({ messages, type }) => {
    if (type !== "notify") return;
    for (const msg of messages) {
      const jid = msg.key.remoteJid;
      if (!jid) continue;
      enqueue(tails, jid, () => onMessage(sock, ira, live, ownSends, outbound, seen, msg));
    }
  });

  await new Promise<void>((resolve, reject) => {
    sock.ev.on("connection.update", (update) => {
      if (update.qr) {
        session.phase = "qr";
        session.user = null;
        void QRCode.toDataURL(update.qr, { margin: 1, width: 280 }).then((url) => {
          session.qr = url;
        });
        console.log("escanea este QR con WhatsApp → Dispositivos vinculados");
        qrcode.generate(update.qr, { small: true });
      }
      if (update.connection === "open") {
        session.phase = "open";
        session.qr = null;
        session.user = sock.user?.id ?? null;
        console.log(`conectado como ${session.user ?? "whatsapp"}`);
        console.log(`auth: ${cfg.authDir}`);
        console.log("escribe en Mensajes a ti mismo");
      }
      if (update.connection === "close") {
        const status = disconnectCode(update.lastDisconnect?.error);
        session.phase = "closed";
        session.sock = null;
        sock.end(undefined);
        if (status === DisconnectReason.loggedOut || session.pairing) {
          void clearSession(cfg.authDir).finally(() => {
            session.pairing = false;
            session.qr = null;
            session.user = null;
            resolve();
          });
          return;
        }
        resolve();
      }
    });
  });
}

async function main(): Promise<void> {
  const cfg = loadConfig();
  if (cfg.pair) {
    await rm(cfg.authDir, { recursive: true, force: true });
    console.log(`sesión borrada: ${cfg.authDir}`);
  }
  await mkdir(cfg.authDir, { recursive: true, mode: 0o700 });
  await tighten(cfg.authDir);
  const live: Live = { allowPhones: await loadAllow(cfg.authDir, cfg.allowPhones) };
  if (live.allowPhones.length === 0) {
    console.warn("allowlist vacía: solo contesto en Mensajes a ti mismo");
  }
  const session: Session = {
    phase: "connecting",
    user: null,
    qr: null,
    pairing: false,
    sock: null,
  };
  await listenControl(cfg.controlPort, {
    status: () => ({
      ok: true,
      phase: session.phase,
      user: session.user,
      qr: session.qr,
      allow_phones: live.allowPhones,
      auth_dir: cfg.authDir,
    }),
    pair: async () => {
      session.pairing = true;
      session.qr = null;
      session.user = null;
      session.phase = "connecting";
      const sock = session.sock;
      if (!sock) {
        await clearSession(cfg.authDir);
        session.pairing = false;
        return;
      }
      try {
        await sock.logout();
      } catch {
        sock.end(undefined);
      }
    },
    setAllow: async (raw) => {
      live.allowPhones = parseAllowPhones(raw);
      await saveAllow(cfg.authDir, live.allowPhones);
      await tighten(cfg.authDir);
    },
  });
  const ira = new IraClient(cfg.apiUrl);
  for (;;) {
    await connect(cfg, ira, live, session);
    console.warn("conexión caída, reintento en 2s");
    await new Promise((resolve) => setTimeout(resolve, 2000));
  }
}

main().catch((error: unknown) => {
  console.error(error instanceof Error ? error.message : error);
  process.exit(1);
});
