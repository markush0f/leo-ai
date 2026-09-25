import { useEffect, useState, type FormEvent } from "react";
import { motion } from "motion/react";
import {
  createDatabase,
  deleteDatabase,
  listDatabases,
  testDatabase,
  updateDatabase,
} from "./api";
import { IconDatabase, IconClose, IconPlus, IconLink, IconCheck } from "./icons";
import { Input, Select } from "./components/Field";
import { Activity } from "./components/Activity";
import { useSheetFocus } from "./components/useSheetFocus";
import { parseDatabaseUrl } from "./database-url";
import type { DatabaseConnection, DatabaseInput, DatabaseTest } from "./types";

const EMPTY: DatabaseInput = {
  name: "",
  host: "",
  port: 5432,
  database: "",
  username: "",
  ssl_mode: "prefer",
  enabled: true,
};

function draftFrom(connection: DatabaseConnection): DatabaseInput {
  return {
    name: connection.name,
    host: connection.host,
    port: connection.port,
    database: connection.database,
    username: connection.username,
    password: "",
    ssl_mode: connection.ssl_mode,
    enabled: connection.enabled,
  };
}

function connectionState(connection: DatabaseConnection): string {
  if (!connection.enabled) return "Desactivada";
  if (connection.last_test_ok === true) return "Última prueba correcta";
  if (connection.last_test_ok === false) return "Revisar conexión";
  return "Sin comprobar";
}

export function Databases({ onClose }: { onClose: () => void }) {
  const sheetRef = useSheetFocus();
  const [items, setItems] = useState<DatabaseConnection[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [creating, setCreating] = useState(false);
  const [draft, setDraft] = useState<DatabaseInput>(EMPTY);
  const [busy, setBusy] = useState(false);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<DatabaseTest | null>(null);
  const [confirmDelete, setConfirmDelete] = useState(false);
  const [connectionUrl, setConnectionUrl] = useState("");
  const [urlError, setUrlError] = useState<string>();
  const [imported, setImported] = useState(false);
  const [phase, setPhase] = useState<"saving" | "testing" | null>(null);

  const selected = items.find((item) => item.id === selectedId) ?? null;

  useEffect(() => {
    void listDatabases()
      .then((connections) => {
        setItems(connections);
        if (connections[0]) {
          setSelectedId(connections[0].id);
          setDraft(draftFrom(connections[0]));
        } else setCreating(true);
      })
      .catch((e) => setError(e instanceof Error ? e.message : String(e)))
      .finally(() => setLoading(false));
  }, []);

  const choose = (connection: DatabaseConnection) => {
    if (busy) return;
    setConnectionUrl(""); setUrlError(undefined); setImported(false);
    setSelectedId(connection.id);
    setCreating(false);
    setDraft(draftFrom(connection));
    setResult(null);
    setError(null);
    setConfirmDelete(false);
  };

  const beginCreate = () => {
    if (busy) return;
    setConnectionUrl(""); setUrlError(undefined); setImported(false);
    setSelectedId(null);
    setCreating(true);
    setDraft({ ...EMPTY });
    setResult(null);
    setError(null);
    setConfirmDelete(false);
  };

  const cancelCreate = () => {
    if (items[0]) choose(items[0]);
    else {
      setCreating(false);
      setDraft({ ...EMPTY });
    }
  };

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    if (busy) return;
    setBusy(true);
    setError(null);
    setResult(null);
    setPhase("saving");
    const input = { ...draft };
    if (!input.password) delete input.password;
    try {
      const saved = creating
        ? await createDatabase(input)
        : await updateDatabase(selectedId as string, input);
      setItems((current) =>
        creating ? [...current, saved] : current.map((item) => (item.id === saved.id ? saved : item)),
      );
      setSelectedId(saved.id);
      setCreating(false);
      setDraft(draftFrom(saved));
      setConnectionUrl("");
      setPhase("testing");
      try {
        const tested = await testDatabase(saved.id);
        setResult(tested);
        setItems((current) => current.map((item) => item.id === saved.id ? {
          ...item, last_test_ok: tested.ok, last_test_error: tested.ok ? null : tested.detail,
          last_tested_at: new Date().toISOString(),
        } : item));
      } catch {
        setError("Conexión guardada, pero no se pudo comprobar. Revisa los datos y vuelve a guardar y comprobar.");
      }
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
      setPhase(null);
    }
  };

  const remove = async () => {
    if (!selectedId) return;
    setBusy(true);
    setError(null);
    try {
      await deleteDatabase(selectedId);
      const remaining = items.filter((item) => item.id !== selectedId);
      setItems(remaining);
      setConfirmDelete(false);
      if (remaining[0]) choose(remaining[0]);
      else beginCreate();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  };

  const importUrl = () => {
    try {
      const parsed = parseDatabaseUrl(connectionUrl);
      setDraft((current) => ({ ...parsed, name: current.name || parsed.name, enabled: current.enabled }));
      setConnectionUrl(""); setUrlError(undefined); setImported(true); setResult(null);
    } catch (error) { setUrlError(error instanceof Error ? error.message : "Revisa la URL."); }
  };

  return (
    <motion.aside
      ref={sheetRef} role="dialog" aria-modal="true" tabIndex={-1}
      className="sheet catalog databases"
      aria-label="Bases de datos"
      aria-busy={busy || loading}
      initial={{ x: "100%" }}
      animate={{ x: 0 }}
      exit={{ x: "100%" }}
      transition={{ type: "spring", stiffness: 360, damping: 38 }}
    >
      <header className="sheet-head">
        <div>
          <h2>Bases de datos</h2>
          <p>Conexiones PostgreSQL disponibles para Ira.</p>
        </div>
        <button type="button" className="btn-ghost icon-button" aria-label="Cerrar bases de datos" title="Cerrar" onClick={onClose}><IconClose /></button>
      </header>

      <div className={`database-notice${creating ? " database-notice-create" : ""}`}>
        <strong>Usa un usuario PostgreSQL de solo lectura.</strong>
        <span>Concede acceso únicamente a los esquemas y tablas necesarios.</span>
      </div>

      <div className={`catalog-layout${creating && items.length === 0 ? " empty-connections" : ""}`}>
        <nav className="catalog-nav" aria-label="Conexiones PostgreSQL">
          <div className="database-nav-head">
            <h3>Conexiones</h3>
            <button type="button" className="btn-secondary sm" disabled={busy || loading} onClick={beginCreate}><IconPlus />Nueva</button>
          </div>
          <ul className="catalog-providers">
            {items.map((item) => (
              <li key={item.id}>
                <button type="button" disabled={busy} aria-pressed={!creating && selectedId === item.id} className={!creating && selectedId === item.id ? "catalog-provider selected" : "catalog-provider"} onClick={() => choose(item)}>
                  <span className="catalog-provider-name">{item.name}</span>
                  <span className="catalog-provider-meta">{item.host}:{item.port} · {connectionState(item)}</span>
                </button>
              </li>
            ))}
          </ul>
          {loading && <p className="catalog-empty" role="status">Cargando conexiones…</p>}
          {!loading && items.length === 0 && <p className="catalog-empty">Tu primera conexión aparecerá aquí.</p>}
        </nav>

        <div className="catalog-detail">
          {(creating || selected) ? (
            <form className={`database-form${creating ? " database-create" : ""}`} onSubmit={(event) => void submit(event)} onChange={() => setResult(null)}
              onInvalid={(event) => { const advanced = (event.target as HTMLElement).closest("details"); if (advanced) advanced.open = true; }}>
              {creating ? (
                <header className="database-create-head">
                  <span className="database-create-icon"><IconDatabase /></span>
                  <div>
                    <h3>Conecta PostgreSQL</h3>
                    <p>Indica dónde está tu base de datos y quién puede leerla.</p>
                    <code>{draft.host || "servidor"}:{draft.port}/{draft.database || "base_de_datos"}</code>
                  </div>
                </header>
              ) : (
                <header className="catalog-detail-head">
                  <h3>{selected?.name}</h3>
                    <p>{selected ? connectionState(selected) : ""} · {selected?.password_set ? "Contraseña guardada" : "Sin contraseña guardada"}</p>
                </header>
              )}
              {selected?.enabled && selected.last_test_ok !== true && selected.last_test_error && <p className="database-result bad" role="alert">{selected.last_test_error}</p>}
              <fieldset className="database-edit-fields" disabled={busy}>
                {creating && <section className="connection-import" aria-label="Importar conexión">
                  <Input label="¿Tienes una URL de conexión?" type="password" icon={<IconLink />} autoComplete="off"
                    placeholder="postgresql://usuario:contraseña@host/base" value={connectionUrl}
                    error={urlError} hint="Pégala para completar los campos. También puedes rellenarlos abajo."
                    onChange={(event) => { setConnectionUrl(event.target.value); setUrlError(undefined); setImported(false); }}
                    onKeyDown={(event) => { if (event.key === "Enter") { event.preventDefault(); importUrl(); } }} />
                  <button type="button" className="btn-secondary" disabled={!connectionUrl.trim()} onClick={importUrl}><IconLink />Completar desde URL</button>
                  {imported && <p className="import-success" role="status"><IconCheck />Datos completados. Puedes revisarlos abajo.</p>}
                </section>}
                <section className="connection-section">
                  <h4>Destino</h4>
                  <Input label="Nombre de la conexión" required placeholder="Mi base de datos" value={draft.name} onChange={(event) => setDraft({ ...draft, name: event.target.value })} />
                  <div className="database-field-row">
                    <Input label="Servidor" required placeholder="db.example.com" value={draft.host} onChange={(event) => setDraft({ ...draft, host: event.target.value })} />
                    <Input label="Base de datos" required placeholder="ira" value={draft.database} onChange={(event) => setDraft({ ...draft, database: event.target.value })} />
                  </div>
                  <p className="connection-help">Desde Docker, usa host.docker.internal para acceder a una base instalada en tu equipo.</p>
                </section>
                <section className="connection-section">
                  <h4>Credenciales</h4>
                  <div className="database-field-row">
                    <Input label="Usuario" required autoComplete="username" placeholder="ira_readonly" value={draft.username} onChange={(event) => setDraft({ ...draft, username: event.target.value })} />
                    <Input label="Contraseña" type="password" autoComplete="new-password" value={draft.password ?? ""} placeholder={selected?.password_set ? "Conservar la guardada" : "Contraseña PostgreSQL"} onChange={(event) => setDraft({ ...draft, password: event.target.value })} />
                  </div>
                </section>
                <details className="connection-advanced">
                  <summary>Opciones avanzadas <span>Puerto {draft.port} · SSL {draft.ssl_mode}</span></summary>
                  <div className="database-field-row">
                    <Input label="Puerto" required type="number" min="1" max="65535" value={draft.port || ""} onChange={(event) => setDraft({ ...draft, port: Number(event.target.value) })} />
                    <Select label="Modo SSL" value={draft.ssl_mode} onChange={(event) => setDraft({ ...draft, ssl_mode: event.target.value })}>
                      <option value="disable">Desactivado</option><option value="prefer">Preferir</option><option value="require">Requerir</option><option value="verify-ca">Verificar CA</option><option value="verify-full">Verificación completa</option>
                    </Select>
                  </div>
                </details>
              </fieldset>
              {creating && <p className="database-create-safety"><strong>Usa acceso de solo lectura</strong><span>Limita este usuario a los esquemas y tablas que Ira necesite consultar.</span></p>}
              <label className="database-enabled"><input type="checkbox" disabled={busy} checked={draft.enabled} onChange={(e) => setDraft({ ...draft, enabled: e.target.checked })} /><span><strong>Disponible para Ira</strong><small>Permitir que las herramientas consulten esta conexión.</small></span></label>
              <div className="database-actions">
                <button type="submit" className="btn-primary" disabled={busy}>{busy ? <Activity /> : <IconCheck />}{phase === "saving" ? "Guardando…" : phase === "testing" ? "Comprobando…" : "Guardar y comprobar"}</button>
                {creating && <button type="button" className="btn-ghost" disabled={busy} onClick={cancelCreate}>Cancelar</button>}
                <span className="save-status" role="status">{phase === "testing" ? "Guardada. Probando acceso a PostgreSQL." : ""}</span>
              </div>
              {result && <p className={result.ok ? "database-result ok" : "database-result bad"} role="status"><strong>{result.ok ? "Conexión correcta" : "Falló la conexión"}</strong><span>{result.detail}</span>{result.ok && !result.read_only && <span>Atención: este usuario no parece ser de solo lectura.</span>}</p>}
              {!creating && (confirmDelete ? <div className="confirm"><span>¿Eliminar {selected?.name}?</span><button type="button" className="btn-danger sm" disabled={busy} onClick={() => void remove()}>Eliminar</button><button type="button" className="btn-secondary sm" onClick={() => setConfirmDelete(false)}>Cancelar</button></div> : <button type="button" className="btn-danger database-delete" disabled={busy} onClick={() => setConfirmDelete(true)}>Eliminar conexión</button>)}
            </form>
          ) : <p className="catalog-empty">Selecciona una conexión o crea una nueva.</p>}
        </div>
      </div>
      {error && <p className="sheet-err" role="alert">{error}</p>}
    </motion.aside>
  );
}
