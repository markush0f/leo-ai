import type { DatabaseInput } from "./types";

const SSL_MODES = new Set(["disable", "prefer", "require", "verify-ca", "verify-full"]);

/** Parses locally; never logs credentials or sends the raw URL to a server. */
export function parseDatabaseUrl(raw: string): DatabaseInput {
  try {
    const url = new URL(raw.trim());
    if (!["postgres:", "postgresql:"].includes(url.protocol)) throw new Error();
    if (!url.hostname || !url.username || url.hash) throw new Error();
    const database = decodeURIComponent(url.pathname.slice(1));
    const port = url.port ? Number(url.port) : 5432;
    if (!database || !Number.isInteger(port) || port < 1 || port > 65535) throw new Error();
    const ssl = url.searchParams.get("sslmode") ?? "prefer";
    if (!SSL_MODES.has(ssl)) throw new Error("ssl");
    if ([...url.searchParams.keys()].some((key) => key !== "sslmode")) throw new Error("options");
    return {
      name: database,
      host: url.hostname.replace(/^\[|\]$/g, ""),
      port, database,
      username: decodeURIComponent(url.username),
      password: decodeURIComponent(url.password),
      ssl_mode: ssl,
      enabled: true,
    };
  } catch (error) {
    if (error instanceof Error && error.message === "ssl") throw new Error("Modo SSL no compatible. Usa disable, prefer, require, verify-ca o verify-full.");
    if (error instanceof Error && error.message === "options") throw new Error("La URL contiene opciones no compatibles. Usa el formulario para revisar la conexión; solo se admite sslmode.");
    throw new Error("Revisa la URL: postgresql://usuario:contraseña@servidor:5432/base. Codifica caracteres especiales de las credenciales.");
  }
}
