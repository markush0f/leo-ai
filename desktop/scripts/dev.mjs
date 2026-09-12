import { execFileSync, spawn } from "node:child_process";

const PORT = Number(process.env.LEO_DEV_PORT || 5179);

try {
  execFileSync("fuser", ["-k", `${PORT}/tcp`], { stdio: "ignore" });
  await new Promise((r) => setTimeout(r, 200));
} catch {
  // nothing was listening
}

const child = spawn("vite", ["--port", String(PORT), "--strictPort"], {
  stdio: "inherit",
  shell: false,
});
child.on("exit", (code) => process.exit(code ?? 0));
