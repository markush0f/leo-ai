export const HELP = [
  "Leo por WhatsApp.",
  "Escríbeme en «Mensajes a ti mismo». No contesto el resto del inbox.",
  "/help  esta ayuda",
  "/status  modelo activo",
  "/clear  nueva conversación",
  "Para enlazar otro número: para este proceso y arranca con --pair.",
].join("\n");

export type Command =
  | { type: "help" }
  | { type: "status" }
  | { type: "clear" }
  | { type: "unknown"; name: string }
  | { type: "chat"; text: string };

export function route(text: string): Command {
  const trimmed = text.trim();
  if (!trimmed.startsWith("/")) return { type: "chat", text: trimmed };
  const head = trimmed.slice(1).split(/\s+/, 1)[0] ?? "";
  const name = head.split("@")[0] ?? "";
  if (!name) return { type: "chat", text: trimmed };
  if (name === "help" || name === "start") return { type: "help" };
  if (name === "status") return { type: "status" };
  if (name === "clear") return { type: "clear" };
  return { type: "unknown", name };
}
