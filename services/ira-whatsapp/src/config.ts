import os from "node:os";
import path from "node:path";
import { parseAllowPhones } from "./allow.ts";

export type Config = {
  pair: boolean;
  authDir: string;
  apiUrl: string;
  allowPhones: string[];
  controlPort: number;
};

export function loadConfig(argv = process.argv.slice(2), env = process.env): Config {
  const home = env.HOME || env.USERPROFILE || os.homedir();
  const authDir =
    env.WHATSAPP_AUTH_DIR?.trim() || path.join(home, ".config", "ira-ai", "whatsapp");
  return {
    pair: argv.includes("--pair"),
    authDir,
    apiUrl: (env.IRA_API_URL?.trim() || "http://127.0.0.1:8787").replace(/\/$/, ""),
    allowPhones: parseAllowPhones(env.WHATSAPP_ALLOW_PHONES),
    controlPort: port(env.WHATSAPP_CONTROL_PORT, 8790),
  };
}

function port(raw: string | undefined, fallback: number): number {
  const value = Number(raw);
  return Number.isInteger(value) && value > 0 && value < 65536 ? value : fallback;
}
