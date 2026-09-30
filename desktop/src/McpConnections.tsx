import { useEffect, useState, type FormEvent } from "react";
import { deleteMcp, listMcp, saveMcp, testMcp } from "./api";
import type { McpInput, McpServer } from "./types";
import { btn, cx } from "./ui";

type Draft = McpInput & { argsText: string; envText: string; headersText: string };

const empty = (): Draft => ({ name: "", transport: "streamable_http", url: "", command: "", args: [], argsText: "[]", env: {}, envText: "{}", headers: {}, headersText: "{}", enabled: true });
const edit = (server: McpServer): Draft => ({
  name: server.name, transport: server.transport, url: server.url ?? "", command: server.command ?? "",
  args: server.args, argsText: JSON.stringify(server.args), env: server.env, envText: JSON.stringify(server.env, null, 2),
  headers: server.headers, headersText: JSON.stringify(server.headers, null, 2), enabled: server.enabled,
});

function stringMap(raw: string): Record<string, string> {
  const value: unknown = JSON.parse(raw);
  if (!value || Array.isArray(value) || typeof value !== "object" || Object.values(value).some((item) => typeof item !== "string")) {
    throw new Error("Usa un objeto JSON con valores de texto.");
  }
  return value as Record<string, string>;
}

const control = "w-full rounded-[11px] border border-line bg-bg px-3 py-2 text-ink";

export function McpConnections() {
  const [servers, setServers] = useState<McpServer[]>([]);
  const [draft, setDraft] = useState<Draft | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState("");
  const [result, setResult] = useState("");
  const reload = async () => setServers(await listMcp());
  useEffect(() => { void reload().catch((reason) => setError(String(reason))); }, []);

  const perform = async (id: string, action: () => Promise<unknown>) => {
    setBusy(id); setError(""); setResult("");
    try { await action(); await reload(); }
    catch (reason) { setError(reason instanceof Error ? reason.message : String(reason)); }
    finally { setBusy(null); }
  };

  const submit = (event: FormEvent) => {
    event.preventDefault();
    if (!draft) return;
    let input: McpInput;
    try {
      const args: unknown = JSON.parse(draft.argsText);
      if (!Array.isArray(args) || args.some((arg) => typeof arg !== "string")) throw new Error("Argumentos debe ser un array JSON de textos.");
      input = { name: draft.name, transport: draft.transport, url: draft.url, command: draft.command,
        args, env: stringMap(draft.envText), headers: stringMap(draft.headersText), enabled: draft.enabled };
    } catch (reason) { setError(reason instanceof Error ? reason.message : String(reason)); return; }
    void perform(draft.name, async () => { await saveMcp(input); setDraft(null); setResult("MCP guardado. Disponible en el próximo turno."); });
  };

  return <section className="mt-5 border-t border-line pt-4" aria-label="MCP externos">
    <div className="flex items-center justify-between gap-3">
      <div><h3 className="m-0 text-[1rem] font-semibold">MCP externos</h3><p className="m-0 text-sm text-muted">Servidores conectados a las conversaciones de Ira.</p></div>
      <button type="button" className={btn.secondary} disabled={busy !== null} onClick={() => { setDraft(empty()); setError(""); setResult(""); }}>Añadir MCP</button>
    </div>
    <p className="mt-2 mb-0 text-xs text-muted">Sin «Escritura», Ira solo usa herramientas MCP marcadas como lectura por su servidor.</p>
    {servers.length === 0 && !draft && <p className="text-sm text-muted">Añade un servidor remoto o un comando local para descubrir sus herramientas.</p>}
    <ul className="mt-3 flex list-none flex-col gap-2 p-0">
      {servers.map((server) => <li key={server.id} className="rounded-[10px] border border-line px-3 py-2">
        <div className="flex flex-wrap items-center gap-2">
          <strong className="min-w-0 break-all">{server.name}</strong>
          <span className="text-xs text-muted">{server.transport === "stdio" ? "Local" : "Remoto"} · {server.enabled ? "Activo" : "Inactivo"}{server.editable ? "" : " · ira.json del proyecto"}</span>
        </div>
        <p className="my-1 break-all text-sm text-muted">{server.url ?? [server.command, ...server.args].join(" ")}</p>
        <div className="flex flex-wrap gap-2">
          <button type="button" className={btn.ghost} disabled={busy !== null} onClick={() => void perform(server.id, async () => {
            const response = await testMcp(server.id);
            setResult(`${server.name}: ${response.tools.length} herramientas${response.tools.length ? ` · ${response.tools.join(", ")}` : ""}`);
          })}>Comprobar</button>
          {server.editable && <>
            <button type="button" className={btn.ghost} disabled={busy !== null} onClick={() => { setDraft(edit(server)); setError(""); }}>Editar</button>
            <button type="button" className={btn.ghost} disabled={busy !== null} onClick={() => void perform(server.id, async () => {
              await saveMcp({ name: server.name, transport: server.transport, url: server.url ?? undefined,
                command: server.command ?? undefined, args: server.args, env: server.env, headers: server.headers,
                enabled: !server.enabled });
            })}>{server.enabled ? "Desactivar" : "Activar"}</button>
            <button type="button" className={cx(btn.ghost, "text-danger")} disabled={busy !== null} onClick={() => {
              if (window.confirm(`¿Eliminar ${server.name}?`)) void perform(server.id, () => deleteMcp(server.id));
            }}>Eliminar</button>
          </>}
        </div>
      </li>)}
    </ul>
    {draft && <form className="mt-4 flex flex-col gap-3 rounded-[12px] bg-elevated p-3" onSubmit={submit}>
      <h4 className="m-0 font-semibold">{servers.some((server) => server.name === draft.name) ? "Editar MCP" : "Nuevo MCP"}</h4>
      <label className="flex flex-col gap-1 text-sm">Nombre<input className={control} value={draft.name} required disabled={servers.some((server) => server.name === draft.name)} onChange={(event) => setDraft({ ...draft, name: event.target.value })} /></label>
      <label className="flex flex-col gap-1 text-sm">Conexión<select className={control} value={draft.transport} onChange={(event) => setDraft({ ...draft, transport: event.target.value as Draft["transport"] })}>
        <option value="streamable_http">HTTP (URL MCP)</option><option value="remote_bridge">SSE / OAuth (requiere npx)</option><option value="stdio">Comando local (stdio)</option>
      </select></label>
      {draft.transport !== "stdio" ? <>
        <label className="flex flex-col gap-1 text-sm">URL completa<input className={control} type="url" required placeholder="https://servidor.example/mcp" value={draft.url} onChange={(event) => setDraft({ ...draft, url: event.target.value })} /></label>
        {draft.transport === "streamable_http" && <label className="flex flex-col gap-1 text-sm">Cabeceras (JSON)<textarea className={control} rows={3} spellCheck={false} value={draft.headersText} onChange={(event) => setDraft({ ...draft, headersText: event.target.value })} /></label>}
      </> : <>
        <label className="flex flex-col gap-1 text-sm">Ejecutable<input className={control} required placeholder="npx" value={draft.command} onChange={(event) => setDraft({ ...draft, command: event.target.value })} /></label>
        <label className="flex flex-col gap-1 text-sm">Argumentos (array JSON)<textarea className={control} rows={2} spellCheck={false} value={draft.argsText} onChange={(event) => setDraft({ ...draft, argsText: event.target.value })} /></label>
        <label className="flex flex-col gap-1 text-sm">Entorno (JSON)<textarea className={control} rows={3} spellCheck={false} value={draft.envText} onChange={(event) => setDraft({ ...draft, envText: event.target.value })} /></label>
      </>}
      <p className="m-0 text-xs text-muted">Usa ${"{SECRET:NOMBRE}"} para credenciales del entorno. Los valores guardados aparecen ocultos; déjalos vacíos para conservarlos.</p>
      <label className="flex items-center gap-2 text-sm"><input type="checkbox" checked={draft.enabled} onChange={(event) => setDraft({ ...draft, enabled: event.target.checked })} />Activo</label>
      <div className="flex gap-2"><button type="submit" className={btn.primary} disabled={busy !== null}>Guardar</button><button type="button" className={btn.ghost} onClick={() => setDraft(null)}>Cancelar</button></div>
    </form>}
    {error && <p className="text-sm text-danger" role="alert">{error}</p>}
    {result && <p className="break-words text-sm text-muted" role="status">{result}</p>}
  </section>;
}
