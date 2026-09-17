import { spawn } from "node:child_process";
import net from "node:net";
import { fileURLToPath } from "node:url";
import path from "node:path";

const PORT = Number(process.env.LEO_DEV_PORT || 5179);
const API_PORT = Number(process.env.LEO_HTTP_PORT || 8787);
const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");

function requireFreePort(port) {
  return new Promise((resolve, reject) => {
    const probe = net.createServer();
    probe.unref();
    probe.once("error", (error) => {
      reject(new Error(`el puerto ${port} está ocupado`, { cause: error }));
    });
    probe.listen(port, "127.0.0.1", () => {
      probe.close(() => resolve());
    });
  });
}

await Promise.all([requireFreePort(PORT), requireFreePort(API_PORT)]);

const apiBind = process.env.LEO_HTTP_BIND || `127.0.0.1:${API_PORT}`;
const server = spawn("cargo", ["run", "-p", "leo-server", "--", "--bind", apiBind], {
  cwd: ROOT,
  stdio: ["ignore", "inherit", "inherit"],
  shell: false,
});

const stop = () => {
  if (server.exitCode === null) server.kill("SIGTERM");
};
process.on("exit", stop);
process.on("SIGINT", () => {
  stop();
  process.exit(130);
});
process.on("SIGTERM", () => {
  stop();
  process.exit(143);
});

const health = `http://127.0.0.1:${API_PORT}/api/health`;
const deadline = Date.now() + 180_000;
let ready = false;
while (Date.now() < deadline) {
  if (server.exitCode !== null) {
    process.exit(server.exitCode || 1);
  }
  try {
    const res = await fetch(health);
    if (res.ok) {
      ready = true;
      break;
    }
  } catch {
    // still compiling or booting
  }
  await new Promise((r) => setTimeout(r, 400));
}
if (!ready) {
  stop();
  console.error(`timeout waiting for ${health}`);
  process.exit(1);
}

const child = spawn("vite", ["--port", String(PORT), "--strictPort"], {
  stdio: "inherit",
  shell: false,
});
child.on("exit", (code) => {
  stop();
  process.exit(code ?? 0);
});
