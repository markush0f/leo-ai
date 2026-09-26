import { chmod, readFile, writeFile } from "node:fs/promises";
import path from "node:path";

const FILE = "allow.json";

export async function loadAllow(dir: string, fallback: string[]): Promise<string[]> {
  try {
    const raw = await readFile(path.join(dir, FILE), "utf8");
    const parsed = JSON.parse(raw) as { phones?: unknown };
    if (!Array.isArray(parsed.phones)) return fallback;
    const phones = parsed.phones.filter(
      (phone): phone is string => typeof phone === "string" && /^\d+$/.test(phone),
    );
    return phones;
  } catch {
    return fallback;
  }
}

export async function saveAllow(dir: string, phones: string[]): Promise<void> {
  const file = path.join(dir, FILE);
  await writeFile(file, `${JSON.stringify({ phones }, null, 2)}\n`, { mode: 0o600 });
  await chmod(file, 0o600);
}
