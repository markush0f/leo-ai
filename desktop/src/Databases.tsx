import { useEffect, useRef, useState, type FormEvent } from "react";
import { createDatabase, deleteDatabase, listDatabases, testDatabase, updateDatabase } from "./api";
import { IconDatabase, IconPlus, IconLink, IconCheck } from "./icons";
import { Input, Select } from "./components/Field";
import { Activity } from "./components/Activity";
import { Status } from "./components/Status";
import { parseDatabaseUrl } from "./database-url";
import { btn } from "./ui";
import type { DatabaseConnection, DatabaseInput, DatabaseTest } from "./types";

const EMPTY: DatabaseInput = { name: "", host: "", port: 5432, database: "", username: "", ssl_mode: "prefer", enabled: true };
function draftFrom(item: DatabaseConnection): DatabaseInput {
  return { name: item.name, host: item.host, port: item.port, database: item.database, username: item.username, password: "", ssl_mode: item.ssl_mode, enabled: item.enabled };
}

export function Databases({ onStart }: { onStart?: () => void }) {
  const [items, setItems] = useState<DatabaseConnection[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState<DatabaseInput>({ ...EMPTY });
  const [phase, setPhase] = useState<"saving" | "testing" | "deleting" | null>(null);
  const [busyId, setBusyId] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const [results, setResults] = useState<Record<string, DatabaseTest>>({});
  const [confirmId, setConfirmId] = useState<string | null>(null);
  const [connectionUrl, setConnectionUrl] = useState("");
  const [urlError, setUrlError] = useState<string>();
  const [manual, setManual] = useState(false);
  const [imported, setImported] = useState(false);
  const editorRef = useRef<HTMLFormElement>(null);
  const openerRef = useRef<HTMLElement | null>(null);
  const selected = items.find((item) => item.id === selectedId);
  const busy = phase !== null;
  useEffect(() => {
    if (editing) editorRef.current?.querySelector<HTMLElement>('input:not(:disabled), select:not(:disabled)')?.focus();
  }, [editing, selectedId]);

  const reload = async () => setItems(await listDatabases());
  useEffect(() => { void reload().catch((e) => setError(String(e))).finally(() => setLoading(false)); }, []);

  const choose = (item?: DatabaseConnection) => {
    openerRef.current = document.activeElement as HTMLElement | null;
    onStart?.();
    setSelectedId(item?.id ?? null); setDraft(item ? draftFrom(item) : { ...EMPTY });
    setManual(Boolean(item)); setImported(false); setConnectionUrl(""); setUrlError(undefined); setError(""); setEditing(true);
  };
  const cancel = () => {
    setEditing(false); setDraft({ ...EMPTY }); setConnectionUrl(""); setUrlError(undefined); setError("");
    requestAnimationFrame(() => openerRef.current?.isConnected && openerRef.current.focus());
  };
  const importUrl = () => {
    try {
      const parsed = parseDatabaseUrl(connectionUrl);
      setDraft({ ...parsed, name: draft.name || parsed.name, enabled: draft.enabled });
      setConnectionUrl(""); setUrlError(undefined); setImported(true);
    } catch (e) { setUrlError(e instanceof Error ? e.message : "Revisa la URL."); }
  };

  const check = async (item: DatabaseConnection) => {
    setPhase("testing"); setBusyId(item.id);
    try {
      const result = await testDatabase(item.id);
      setResults((current) => ({ ...current, [item.id]: result }));
      setItems((current) => current.map((value) => value.id === item.id ? { ...value, last_test_ok: result.ok, last_test_error: result.ok ? null : result.detail, last_tested_at: new Date().toISOString() } : value));
      return result;
    } catch {
      const result = { ok: false, read_only: false, detail: "No se pudo comprobar el acceso. Revisa la conexión y reintenta." };
      setResults((current) => ({ ...current, [item.id]: result }));
      return result;
    } finally { setPhase(null); setBusyId(null); }
  };

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    if (busy) return;
    let input = { ...draft };
    if (connectionUrl.trim() || (!manual && !imported && !selectedId)) {
      try { input = { ...parseDatabaseUrl(connectionUrl), enabled: draft.enabled }; }
      catch (e) { setUrlError(e instanceof Error ? e.message : "Revisa la URL."); return; }
    }
    if (!input.password) delete input.password;
    setPhase("saving"); setError("");
    try {
      const saved = selectedId ? await updateDatabase(selectedId, input) : await createDatabase(input);
      setItems((current) => [...current.filter((item) => item.id !== saved.id), saved]);
      setSelectedId(saved.id); setDraft(draftFrom(saved)); setConnectionUrl(""); setImported(false); setManual(true);
      const result = await check(saved);
      if (result.ok) cancel();
      else requestAnimationFrame(() => editorRef.current?.querySelector<HTMLElement>('input:not(:disabled), select:not(:disabled)')?.focus());
    } catch (e) { setError(e instanceof Error ? e.message : String(e)); }
    finally { setPhase(null); }
  };

  const remove = async (item: DatabaseConnection) => {
    setPhase("deleting"); setBusyId(item.id);
    try { await deleteDatabase(item.id); setItems((current) => current.filter((value) => value.id !== item.id)); setConfirmId(null); if (selectedId === item.id) cancel(); }
    catch (e) { setResults((current) => ({ ...current, [item.id]: { ok: false, read_only: false, detail: e instanceof Error ? e.message : String(e) } })); }
    finally { setPhase(null); setBusyId(null); }
  };

  return <section className="resource-section" aria-label="Bases de datos">
    <div className="resource-heading"><div><h2>Bases de datos</h2><p>PostgreSQL que Ira puede consultar. Separadas de su almacenamiento interno.</p></div><button type="button" className={btn.secondary} disabled={loading || busy || editing} onClick={() => choose()}><IconPlus />Añadir base de datos</button></div>
    {loading && <div role="status" aria-label="Cargando bases de datos"><div className="loading-skeleton" /></div>}
    {!loading && items.length === 0 && !editing && !error && <div className="empty-state"><IconDatabase /><h3>Pregunta sobre tus propios datos</h3><p>Pega una URL PostgreSQL y comprueba acceso. También puedes introducir los datos de conexión manualmente.</p><button type="button" className={btn.secondary} onClick={() => choose()}>Conectar PostgreSQL</button></div>}
    <ul className="resource-list">
      {items.map((item) => {
        const result = results[item.id];
        const ok = result?.ok ?? item.last_test_ok;
        const detail = result?.detail ?? item.last_test_error;
        return <li key={item.id} className="resource-row">
          <div className="resource-top"><span className="resource-icon"><IconDatabase /></span><div className="resource-info"><strong>{item.name}</strong><p className="resource-description">{item.database} · {item.host}</p>
            <Status busy={busyId === item.id} tone={!item.enabled ? "neutral" : ok === true ? "success" : ok === false ? "error" : "neutral"}>{busyId === item.id ? "Comprobando…" : !item.enabled ? "Desactivada" : ok === true ? "Última prueba correcta" : ok === false ? "Necesita atención" : "Sin comprobar"}</Status>
          </div><div className="resource-actions"><button type="button" className={btn.secondary} disabled={busy} onClick={() => void check(item)}>{ok === false ? "Reintentar" : "Comprobar"}</button><button type="button" className={btn.ghost} disabled={busy} onClick={() => choose(item)}>Configurar</button></div></div>
          {detail && <p className={`resource-result ${ok === false ? "text-danger" : "text-muted"}`} role={ok === false ? "alert" : "status"}>{ok === false ? "Falló la conexión. " : "Conexión correcta. "}{detail}</p>}
          {result?.ok && !result.read_only && <p className="resource-result text-muted">Este usuario no parece ser de solo lectura.</p>}
          <details className="advanced"><summary>Detalles y acciones</summary><div className="advanced-body"><p className="resource-description">Usuario {item.username} · Puerto {item.port} · SSL {item.ssl_mode}{item.last_tested_at ? ` · Comprobado ${new Date(item.last_tested_at).toLocaleString()}` : ""}</p><div className="form-actions">
            {confirmId === item.id ? <><span className="text-sm">¿Eliminar {item.name}?</span><button type="button" className={btn.danger} disabled={busy} onClick={() => void remove(item)}>Confirmar eliminación</button><button type="button" className={btn.ghost} disabled={busy} onClick={() => setConfirmId(null)}>Cancelar</button></> : <button type="button" className={btn.danger} disabled={busy} onClick={() => setConfirmId(item.id)}>Eliminar conexión</button>}
          </div></div></details>
        </li>;
      })}
    </ul>
    {editing && <form ref={editorRef} className="resource-editor" onSubmit={(e) => void submit(e)} onInvalid={(event) => { const details = (event.target as HTMLElement).closest("details"); if (details) details.open = true; }}>
      <h3>{selected ? `Configurar ${selected.name}` : "Conecta PostgreSQL"}</h3>
      <p className="resource-description mb-5">Usa un usuario de solo lectura para las tablas que quieras consultar.</p>
      <fieldset disabled={busy}>
        {!selectedId && <>
          <Input label="URL de conexión PostgreSQL" type="password" icon={<IconLink />} autoComplete="off" required={!manual && !imported} placeholder="postgresql://usuario:contraseña@host/base" value={connectionUrl} error={urlError}
            hint="Una URL nueva sustituye los datos importados antes de guardar." onChange={(e) => { setConnectionUrl(e.target.value); setUrlError(undefined); setImported(false); }} />
          <div className="form-actions mb-5"><button type="button" className={btn.secondary} disabled={!connectionUrl.trim()} onClick={importUrl}>Revisar datos de URL</button><button type="button" className={btn.ghost} onClick={() => { setManual(!manual); setImported(false); setConnectionUrl(""); setUrlError(undefined); }}>{manual ? "Usar URL de conexión" : "Introducir datos manualmente"}</button></div>
        </>}
        {imported && <div className="mb-4" role="status"><Status tone="success">Datos importados</Status><p className="resource-description">{draft.database} en {draft.host}:{draft.port} · Usuario {draft.username}</p></div>}
        {(manual || imported || selectedId) && <details className="advanced" open={manual || Boolean(selectedId)}><summary>Editar datos de conexión</summary><div className="advanced-body">
          <Input label="Nombre de la conexión" required value={draft.name} placeholder="Mi base de datos" onChange={(e) => setDraft({ ...draft, name: e.target.value })} />
          <div className="grid grid-cols-2 gap-3 phone:grid-cols-1"><Input label="Servidor" required value={draft.host} placeholder="db.example.com" onChange={(e) => setDraft({ ...draft, host: e.target.value })} /><Input label="Base de datos" required value={draft.database} onChange={(e) => setDraft({ ...draft, database: e.target.value })} /></div>
          <div className="grid grid-cols-2 gap-3 phone:grid-cols-1"><Input label="Usuario" required autoComplete="username" value={draft.username} onChange={(e) => setDraft({ ...draft, username: e.target.value })} /><Input label="Contraseña" type="password" autoComplete="new-password" placeholder={selected?.password_set ? "Vacía conserva la guardada" : "Contraseña PostgreSQL"} value={draft.password ?? ""} onChange={(e) => setDraft({ ...draft, password: e.target.value })} /></div>
          <details className="advanced"><summary>Opciones avanzadas · puerto y SSL</summary><div className="advanced-body grid grid-cols-2 gap-3 phone:grid-cols-1"><Input label="Puerto" type="number" required min="1" max="65535" value={draft.port || ""} onChange={(e) => setDraft({ ...draft, port: Number(e.target.value) })} /><Select label="Modo SSL" value={draft.ssl_mode} onChange={(e) => setDraft({ ...draft, ssl_mode: e.target.value })}><option value="disable">Desactivado</option><option value="prefer">Preferir</option><option value="require">Requerir</option><option value="verify-ca">Verificar CA</option><option value="verify-full">Verificación completa</option></Select><p className="resource-description col-span-full">Desde Docker, host.docker.internal permite acceder a tu equipo cuando está configurado por el despliegue.</p></div></details>
        </div></details>}
        <label className="mt-5 flex items-center gap-2 text-sm"><input type="checkbox" checked={draft.enabled} onChange={(e) => setDraft({ ...draft, enabled: e.target.checked })} />Disponible para Ira</label>
      </fieldset>
      <div className="form-actions"><button type="submit" className={btn.primary} disabled={busy}>{busy ? <Activity /> : <IconCheck />}{phase === "saving" ? "Guardando…" : phase === "testing" ? "Comprobando…" : "Guardar y comprobar"}</button><button type="button" className={btn.ghost} disabled={busy} onClick={cancel}>Cancelar</button></div>
      {selectedId && results[selectedId]?.ok === false && <p className="resource-result text-danger" role="alert">Guardada, pero acceso no verificado. Corrige los datos y vuelve a guardar y comprobar.</p>}
    </form>}
    {error && <div className="resource-result text-danger" role="alert">{error}{!editing && <button type="button" className={btn.secondary} onClick={() => { setError(""); setLoading(true); void reload().catch((e) => setError(String(e))).finally(() => setLoading(false)); }}>Reintentar carga</button>}</div>}
  </section>;
}
