import { StrictMode, useEffect, useState, type FormEvent } from "react";
import { createRoot } from "react-dom/client";
import "@fontsource-variable/source-sans-3";
import App from "./App";
import { inTauri } from "./api";
import { Input } from "./components/Field";
import { btn } from "./ui";
import "./styles.css";

function WebEntry() {
  const [state, setState] = useState<"loading" | "login" | "ready" | "error">(inTauri ? "ready" : "loading");
  const [password, setPassword] = useState("");
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    if (inTauri) return;
    void fetch("/api/auth/status")
      .then((response) => setState(response.ok ? "ready" : response.status === 401 ? "login" : "error"))
      .catch(() => setState("error"));
  }, []);

  async function signIn(event: FormEvent) {
    event.preventDefault();
    setBusy(true);
    setError("");
    try {
      const response = await fetch("/api/auth/login", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ password }),
      });
      if (!response.ok) throw new Error(response.status === 401 ? "Contraseña incorrecta." : response.status === 429 ? "Demasiados intentos. Espera cinco minutos." : "No se pudo iniciar sesión.");
      setPassword("");
      setState("ready");
    } catch (err) {
      setError(err instanceof Error ? err.message : "No se pudo iniciar sesión.");
    } finally {
      setBusy(false);
    }
  }

  async function signOut() {
    try {
      const response = await fetch("/api/auth/logout", { method: "POST" });
      if (!response.ok) throw new Error("No se pudo cerrar sesión.");
      setState("login");
    } catch (err) {
      window.alert(err instanceof Error ? err.message : "No se pudo cerrar sesión.");
    }
  }

  if (state === "ready") return <App onLogout={inTauri ? undefined : signOut} />;
  return <main className="flex min-h-dvh items-center justify-center bg-bg px-5 py-10 text-ink">
    <div className="w-full max-w-[25rem]">
      <img className="mb-5 h-20 w-20 object-contain" src="/ira-cabeza-recortada.png" alt="" />
      <h1 className="mb-2 text-[2rem] font-semibold tracking-[-0.03em]">Entra en Ira</h1>
      {state === "loading" && <p className="text-muted" role="status">Comprobando acceso…</p>}
      {state === "error" && <p className="text-danger" role="alert">No se pudo conectar con Ira. Comprueba que el servidor esté activo y recarga la página.</p>}
      {state === "login" && <form onSubmit={(event) => void signIn(event)} className="mt-7 flex flex-col gap-5">
        <p className="m-0 text-muted">Introduce tu contraseña para continuar.</p>
        <Input label="Contraseña" type="password" value={password} onChange={(event) => setPassword(event.target.value)}
          required autoComplete="current-password" autoFocus disabled={busy} error={error || undefined} />
        <button className={`${btn.primary} min-h-12 w-full`} type="submit" disabled={busy || !password}>
          {busy ? "Comprobando…" : "Entrar"}
        </button>
      </form>}
    </div>
  </main>;
}

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <WebEntry />
  </StrictMode>,
);
