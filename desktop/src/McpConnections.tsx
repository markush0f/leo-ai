import { useEffect, useRef, useState, type FormEvent } from "react";
import { deleteMcp, listMcp, saveMcp, testMcp } from "./api";
import type { McpInput, McpServer } from "./types";
import { Input, Select, TextArea } from "./components/Field";
import { Activity } from "./components/Activity";
import { Status } from "./components/Status";
import { IconLink, IconPlus, IconTools, IconDelete } from "./icons";
import { btn, cx } from "./ui";

type Pair = { key: string; value: string };
type Draft = Omit<McpInput, "env" | "headers"> & { env: Pair[]; headers: Pair[] };
type Check = { tools?: string[]; error?: string };
const pairs = (value: Record<string, string>): Pair[] => Object.entries(value).map(([key, value]) => ({ key, value }));
const empty = (): Draft => ({ name: "", transport: "streamable_http", url: "", command: "", args: [], env: [], headers: [], enabled: true });

function record(items: Pair[]): Record<string, string> {
  const out: Record<string, string> = {};
  for (const item of items) {
    if (!item.key.trim() && !item.value) continue;
    if (!item.key.trim()) throw new Error("Indica un nombre para cada valor de autenticación o variable.");
    if (Object.prototype.hasOwnProperty.call(out, item.key.trim())) throw new Error("Hay nombres repetidos. Usa un nombre distinto en cada fila.");
    Object.defineProperty(out, item.key.trim(), { value: item.value, enumerable: true });
  }
  return out;
}

function PairFields({ label, items, onChange }: { label: string; items: Pair[]; onChange: (items: Pair[]) => void }) {
  return <div className="mb-4">
    <h4 className="mb-3 font-semibold text-sm">{label}</h4>
    {items.map((item, index) => <div key={index} className="grid grid-cols-[minmax(0,1fr)_minmax(0,1fr)_44px] gap-2 phone:grid-cols-[minmax(0,1fr)_44px]">
      <div className="phone:col-span-2"><Input label={`${label}: nombre ${index + 1}`} value={item.key} placeholder={label === "Cabeceras" ? "Authorization" : "API_KEY"} onChange={(e) => onChange(items.map((pair, i) => i === index ? { ...pair, key: e.target.value } : pair))} /></div>
      <Input label={`${label}: valor ${index + 1}`} type="password" autoComplete="off" value={item.value} placeholder="Vacío conserva el guardado" onChange={(e) => onChange(items.map((pair, i) => i === index ? { ...pair, value: e.target.value } : pair))} />
      <button type="button" className={cx(btn.icon, "mt-6")} aria-label={`Quitar ${label.toLowerCase()} ${index + 1}`} onClick={() => onChange(items.filter((_, i) => i !== index))}><IconDelete /></button>
    </div>)}
    <button type="button" className={btn.secondary} onClick={() => onChange([...items, { key: "", value: "" }])}><IconPlus />Añadir {label === "Cabeceras" ? "cabecera" : "variable"}</button>
  </div>;
}

export function McpConnections({ onStart }: { onStart?: () => void }) {
  const [servers, setServers] = useState<McpServer[]>([]);
  const [loading, setLoading] = useState(true);
  const [draft, setDraft] = useState<Draft | null>(null);
  const [editingId, setEditingId] = useState<string | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [checkingId, setCheckingId] = useState<string | null>(null);
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  const [checks, setChecks] = useState<Record<string, Check>>({});
  const [confirmId, setConfirmId] = useState<string | null>(null);
  const [raw, setRaw] = useState("");
  const [checkAnnouncement, setCheckAnnouncement] = useState("");
  const editorRef = useRef<HTMLFormElement>(null);
  const openerRef = useRef<HTMLElement | null>(null);
  const editorOpen = draft !== null;
  useEffect(() => {
    if (editorOpen && busy === null) editorRef.current?.querySelector<HTMLElement>('input:not(:disabled), select:not(:disabled)')?.focus();
  }, [editorOpen, editingId, busy]);
  const begin = () => { openerRef.current = document.activeElement as HTMLElement | null; onStart?.(); };
  const closeEditor = () => {
    setDraft(null); setEditingId(null); setError(""); setRaw("");
    requestAnimationFrame(() => openerRef.current?.isConnected && openerRef.current.focus());
  };
  const reload = async () => setServers(await listMcp());
  useEffect(() => { void reload().catch((e) => setError(String(e))).finally(() => setLoading(false)); }, []);

  const check = async (server: McpServer) => {
    setBusy(server.id);
    setCheckingId(server.id);
    setCheckAnnouncement(`${server.name}: comprobando herramientas.`);
    setChecks((current) => ({ ...current, [server.id]: {} }));
    try {
      const response = await testMcp(server.id);
      setChecks((current) => ({ ...current, [server.id]: { tools: response.tools } }));
      setCheckAnnouncement(`${server.name}: conexión comprobada. ${response.tools.length} herramientas descubiertas.`);
      return true;
    } catch (e) {
      setChecks((current) => ({ ...current, [server.id]: { error: e instanceof Error ? e.message : String(e) } }));
      setCheckAnnouncement(`${server.name}: falló la comprobación. Revisa la configuración y reintenta.`);
      return false;
    } finally { setBusy(null); setCheckingId(null); }
  };

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    if (!draft || busy) return;
    setError(""); setNotice("");
    let input: McpInput;
    try { input = { ...draft, name: draft.name.trim(), env: record(draft.env), headers: record(draft.headers) }; }
    catch (e) { setError(e instanceof Error ? e.message : String(e)); return; }
    setBusy("saving");
    try {
      const saved = await saveMcp(input);
      setServers((current) => [...current.filter((server) => server.id !== saved.id), saved]);
      setEditingId(saved.id); setRaw("");
      setNotice("Conexión guardada. Comprobando herramientas…");
      if (saved.enabled) {
        const ok = await check(saved);
        if (ok) closeEditor();
        setNotice("Conexión guardada. El resultado aparece junto al servidor MCP.");
      } else { closeEditor(); setNotice("Conexión guardada y desactivada."); }
    } catch (e) { setError(e instanceof Error ? e.message : String(e)); }
    finally { setBusy(null); }
  };

  const toggle = async (server: McpServer) => {
    setBusy(server.id); setError("");
    try {
      const saved = await saveMcp({ ...server, url: server.url ?? undefined, command: server.command ?? undefined, enabled: !server.enabled });
      setServers((current) => current.map((item) => item.id === server.id ? saved : item));
      setChecks((current) => ({ ...current, [server.id]: {} }));
    } catch (e) { setChecks((current) => ({ ...current, [server.id]: { error: e instanceof Error ? e.message : String(e) } })); }
    finally { setBusy(null); }
  };

  const remove = async (server: McpServer) => {
    setBusy(server.id);
    try { await deleteMcp(server.id); setServers((current) => current.filter((item) => item.id !== server.id)); setConfirmId(null); }
    catch (e) { setChecks((current) => ({ ...current, [server.id]: { error: e instanceof Error ? e.message : String(e) } })); }
    finally { setBusy(null); }
  };

  const importConfig = () => {
    try {
      const parsed: unknown = JSON.parse(raw);
      if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) throw new Error("Pega el objeto de configuración de un servidor MCP.");
      const config = parsed as Record<string, unknown>;
      const map = (value: unknown): Record<string, string> => {
        if (value === undefined) return {};
        if (!value || typeof value !== "object" || Array.isArray(value) || Object.values(value).some((v) => typeof v !== "string")) throw new Error("Variables y cabeceras deben contener valores de texto.");
        return value as Record<string, string>;
      };
      if (config.args !== undefined && (!Array.isArray(config.args) || config.args.some((arg) => typeof arg !== "string"))) throw new Error("Los argumentos deben ser una lista de textos.");
      if (typeof config.url !== "string" && typeof config.command !== "string") throw new Error("La configuración necesita una URL o un comando.");
      setDraft((current) => current && ({ ...current, transport: typeof config.command === "string" ? "stdio" : "streamable_http",
        url: typeof config.url === "string" ? config.url : "", command: typeof config.command === "string" ? config.command : "",
        args: (config.args ?? []) as string[], env: pairs(map(config.env)), headers: pairs(map(config.headers)) }));
      setRaw(""); setError("");
    } catch (e) { setError(e instanceof Error ? e.message : String(e)); }
  };

  return <section className="resource-section" aria-label="Servidores MCP">
    <div className="resource-heading">
      <div><h2>Servidores MCP</h2><p>Conecta herramientas que Ira puede usar en tus conversaciones.</p></div>
      <button type="button" className={btn.secondary} disabled={loading || busy !== null || draft !== null} onClick={() => { begin(); setDraft(empty()); setEditingId(null); setError(""); setNotice(""); setRaw(""); }}><IconPlus />Añadir servidor MCP</button>
    </div>
    {loading && <div role="status" aria-label="Cargando servidores MCP"><div className="loading-skeleton" /><div className="loading-skeleton" /></div>}
    {!loading && servers.length === 0 && !draft && !error && <div className="empty-state"><IconLink /><h3>Más posibilidades para Ira</h3><p>Añade la URL de un servidor MCP para conectar sus herramientas. También puedes ejecutar uno instalado en tu equipo.</p><button type="button" className={btn.primary} onClick={() => { begin(); setDraft(empty()); setEditingId(null); }}>Conectar mi primer servidor MCP</button></div>}
    <ul className="resource-list">
      {servers.map((server) => {
        const result = checks[server.id];
        const checking = checkingId === server.id;
        const status = busy === server.id ? checking ? "Comprobando…" : "Aplicando cambio…" : !server.enabled ? "Desactivado" : result?.error ? "Necesita atención" : result?.tools ? "Última prueba correcta" : "Sin comprobar";
        return <li key={server.id} className="resource-row">
          <div className="resource-top">
            <span className="resource-icon"><IconTools /></span>
            <div className="resource-info"><strong>{server.name}</strong><p className="resource-description">{server.transport === "stdio" ? "Herramientas en tu equipo" : "Servidor remoto"}{server.editable ? "" : " · Configurado en ira.json"}</p>
              <Status busy={checking} tone={server.enabled && result?.error ? "error" : server.enabled && result?.tools ? "success" : "neutral"}>{status}</Status>
            </div>
            <div className="resource-actions">
              <button type="button" className={btn.secondary} disabled={busy !== null || !server.enabled} onClick={() => void check(server)}>{result?.error ? "Reintentar" : "Comprobar"}</button>
              {server.editable && <button type="button" className={btn.ghost} disabled={busy !== null} onClick={() => {
                begin();
                setDraft({ ...server, url: server.url ?? "", command: server.command ?? "", env: pairs(server.env), headers: pairs(server.headers) });
                setEditingId(server.id); setError(""); setRaw("");
              }}>Configurar</button>}
            </div>
          </div>
          {result?.error && <p className="resource-result text-danger" role="alert">{result.error} Revisa la configuración y vuelve a comprobar.</p>}
          {result?.tools && <details className="advanced"><summary>{result.tools.length} {result.tools.length === 1 ? "herramienta descubierta" : "herramientas descubiertas"}</summary><ul className="advanced-body text-sm text-muted">{result.tools.map((tool) => <li className="wrap-anywhere" key={tool}>{tool}</li>)}</ul>{result.tools.length === 0 && <p className="text-sm text-muted">El servidor responde, pero no ofrece herramientas.</p>}</details>}
          <details className="advanced"><summary>Detalles y acciones</summary><div className="advanced-body">
            <p className="resource-description">{server.url ?? [server.command, ...server.args].join(" ")}</p>
            {server.editable && <div className="form-actions"><button type="button" className={btn.secondary} disabled={busy !== null} onClick={() => void toggle(server)}>{server.enabled ? "Desactivar" : "Activar"}</button>
              {confirmId === server.id ? <><span className="text-sm">¿Eliminar {server.name}?</span><button type="button" className={btn.danger} disabled={busy !== null} onClick={() => void remove(server)}>Confirmar eliminación</button><button type="button" className={btn.ghost} disabled={busy !== null} onClick={() => setConfirmId(null)}>Cancelar</button></> : <button type="button" className={btn.danger} disabled={busy !== null} onClick={() => setConfirmId(server.id)}>Eliminar</button>}
            </div>}
          </div></details>
        </li>;
      })}
    </ul>
    {draft && <form ref={editorRef} className="resource-editor" onSubmit={(e) => void submit(e)} onInvalid={(event) => { const details = (event.target as HTMLElement).closest("details"); if (details) details.open = true; }}>
      <h3>{editingId ? "Configurar servidor MCP" : "Conectar un servidor MCP"}</h3>
      <p className="resource-description mb-5">Guarda la conexión y comprobaremos qué herramientas ofrece.</p>
      <fieldset disabled={busy !== null}>
        <Input label="Nombre del servidor MCP" required value={draft.name} disabled={editingId !== null} placeholder="Por ejemplo, mis tareas" onChange={(e) => setDraft({ ...draft, name: e.target.value })} />
        <Select label="Dónde está el servidor" value={draft.transport} onChange={(e) => setDraft({ ...draft, transport: e.target.value as Draft["transport"] })}>
          <option value="streamable_http">En internet · URL MCP</option><option value="stdio">En mi equipo · comando local</option><option value="remote_bridge">Conexión SSE / OAuth · requiere npx</option>
        </Select>
        {draft.transport !== "stdio" ? <>
          <Input label="URL del servidor MCP" type="url" required placeholder="https://servidor.example/mcp" value={draft.url} onChange={(e) => setDraft({ ...draft, url: e.target.value })} />
          {draft.transport === "streamable_http" && <details className="advanced" open={draft.headers.length > 0}><summary>Autenticación y cabeceras</summary><div className="advanced-body"><PairFields label="Cabeceras" items={draft.headers} onChange={(headers) => setDraft({ ...draft, headers })} /></div></details>}
          {draft.transport === "remote_bridge" && <p className="resource-description">La autorización la gestiona el puente local. Necesita Node y npx disponibles en el equipo de Ira.</p>}
        </> : <>
          <Input label="Ejecutable" required placeholder="npx" value={draft.command} onChange={(e) => setDraft({ ...draft, command: e.target.value })} />
          <h4 className="mb-3 text-sm font-semibold">Argumentos, en orden</h4>
          {draft.args.map((arg, i) => <div key={i} className="flex items-start gap-2"><div className="min-w-0 flex-1"><Input label={`Argumento ${i + 1}`} value={arg} onChange={(e) => setDraft({ ...draft, args: draft.args.map((value, index) => index === i ? e.target.value : value) })} /></div><button type="button" className={cx(btn.icon, "mt-6")} aria-label={`Quitar argumento ${i + 1}`} onClick={() => setDraft({ ...draft, args: draft.args.filter((_, index) => index !== i) })}><IconDelete /></button></div>)}
          <button type="button" className={btn.secondary} onClick={() => setDraft({ ...draft, args: [...draft.args, ""] })}><IconPlus />Añadir argumento</button>
          <details className="advanced" open={draft.env.length > 0}><summary>Variables de entorno</summary><div className="advanced-body"><PairFields label="Variables" items={draft.env} onChange={(env) => setDraft({ ...draft, env })} /></div></details>
        </>}
        <details className="advanced"><summary>Avanzado · importar configuración JSON</summary><div className="advanced-body"><TextArea label="Configuración de un servidor" rows={4} value={raw} onChange={(e) => setRaw(e.target.value)} spellCheck={false} /><button type="button" className={btn.secondary} disabled={!raw.trim()} onClick={importConfig}>Rellenar desde JSON</button></div></details>
        <p className="resource-description mt-4">Puedes usar ${"{SECRET:NOMBRE}"} para credenciales del entorno. Un valor vacío conserva el secreto guardado para ese nombre.</p>
        <label className="mt-4 flex items-center gap-2 text-sm"><input type="checkbox" checked={draft.enabled} onChange={(e) => setDraft({ ...draft, enabled: e.target.checked })} />Disponible para Ira</label>
      </fieldset>
      <div className="form-actions"><button type="submit" className={btn.primary} disabled={busy !== null}>{busy ? <Activity /> : <IconLink />}{checkingId ? "Comprobando…" : busy ? "Guardando…" : "Guardar y comprobar"}</button><button type="button" className={btn.ghost} disabled={busy !== null} onClick={closeEditor}>Cancelar</button></div>
    </form>}
    {error && <div className="resource-result text-danger" role="alert">{error}{!draft && <button type="button" className={btn.secondary} disabled={busy !== null} onClick={() => { setLoading(true); setError(""); void reload().catch((e) => setError(String(e))).finally(() => setLoading(false)); }}>Reintentar carga</button>}</div>}
    {notice && <p className="resource-result text-muted" role="status">{notice}</p>}
    <p className="sr-only" role="status" aria-live="polite" aria-atomic="true">{checkAnnouncement}</p>
    <p className="resource-description mt-4">Sin permiso de escritura, Ira usa únicamente herramientas marcadas como lectura por el servidor.</p>
  </section>;
}
