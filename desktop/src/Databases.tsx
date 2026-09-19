import { useEffect, useState, type FormEvent } from "react";
import {
  createDatabase,
  deleteDatabase,
  listDatabases,
  testDatabase,
  updateDatabase,
} from "./api";
import { IconDatabase } from "./icons";
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
  if (connection.last_test_ok === true) return "Activa";
  if (connection.last_test_ok === false) return "Error";
  return "Pendiente";
}

export function Databases({ onClose }: { onClose: () => void }) {
  const [items, setItems] = useState<DatabaseConnection[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [creating, setCreating] = useState(false);
  const [draft, setDraft] = useState<DatabaseInput>(EMPTY);
  const [busy, setBusy] = useState(false);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<DatabaseTest | null>(null);
  const [confirmDelete, setConfirmDelete] = useState(false);

  const selected = items.find((item) => item.id === selectedId) ?? null;

  useEffect(() => {
    void listDatabases()
      .then((connections) => {
        setItems(connections);
        if (connections[0]) {
          setSelectedId(connections[0].id);
          setDraft(draftFrom(connections[0]));
        }
      })
      .catch((e) => setError(e instanceof Error ? e.message : String(e)))
      .finally(() => setLoading(false));
  }, []);

  const choose = (connection: DatabaseConnection) => {
    setSelectedId(connection.id);
    setCreating(false);
    setDraft(draftFrom(connection));
    setResult(null);
    setError(null);
    setConfirmDelete(false);
  };

  const beginCreate = () => {
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
    setBusy(true);
    setError(null);
    setResult(null);
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
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
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

  const test = async () => {
    if (!selectedId) return;
    setBusy(true);
    setError(null);
    setResult(null);
    try {
      const tested = await testDatabase(selectedId);
      setResult(tested);
      setItems(await listDatabases());
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <aside className="sheet catalog databases" aria-label="Bases de datos" aria-busy={busy || loading}>
      <header className="sheet-head">
        <div>
          <h2>Bases de datos</h2>
          <p>Conexiones PostgreSQL disponibles para Leo.</p>
        </div>
        <button type="button" className="btn-ghost" onClick={onClose}>Cerrar</button>
      </header>

      <div className={`database-notice${creating ? " database-notice-create" : ""}`}>
        <strong>Usa un usuario PostgreSQL de solo lectura.</strong>
        <span>Concede acceso únicamente a los esquemas y tablas necesarios.</span>
      </div>

      <div className="catalog-layout">
        <nav className="catalog-nav" aria-label="Conexiones PostgreSQL">
          <div className="database-nav-head">
            <h3>Conexiones</h3>
            <button type="button" className="btn-secondary sm" onClick={beginCreate}>Nueva</button>
          </div>
          <ul className="catalog-providers">
            {items.map((item) => (
              <li key={item.id}>
                <button type="button" className={!creating && selectedId === item.id ? "catalog-provider selected" : "catalog-provider"} onClick={() => choose(item)}>
                  <span className="catalog-provider-name">{item.name}</span>
                  <span className="catalog-provider-meta">{item.host}:{item.port} · {connectionState(item)}</span>
                </button>
              </li>
            ))}
          </ul>
          {!loading && items.length === 0 && !creating && <p className="catalog-empty">No hay conexiones.</p>}
        </nav>

        <div className="catalog-detail">
          {(creating || selected) ? (
            <form className={`database-form${creating ? " database-create" : ""}`} onSubmit={(event) => void submit(event)}>
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
                  <p>{selected?.password_set ? "Contraseña guardada" : "Sin contraseña guardada"}</p>
                </header>
              )}
              {selected?.enabled && selected.last_test_ok !== true && selected.last_test_error && <p className="database-result bad" role="alert">{selected.last_test_error}</p>}
              {creating ? (
                <div className="database-create-fields">
                  <fieldset>
                    <legend>Identidad</legend>
                    <div className="database-field-row">
                      <label className="field"><span>Nombre de la conexión</span><input required autoFocus placeholder="Producción" value={draft.name} onChange={(e) => setDraft({ ...draft, name: e.target.value })} /></label>
                      <label className="field"><span>Base de datos</span><input required placeholder="leo" value={draft.database} onChange={(e) => setDraft({ ...draft, database: e.target.value })} /></label>
                    </div>
                  </fieldset>
                  <fieldset>
                    <legend>Servidor</legend>
                    <div className="database-field-row database-server-row">
                      <label className="field"><span>Host</span><input required placeholder="db.example.com" value={draft.host} onChange={(e) => setDraft({ ...draft, host: e.target.value })} /><small>Para una base local desde Docker, usa host.docker.internal.</small></label>
                      <label className="field"><span>Puerto</span><input required type="number" min="1" max="65535" value={draft.port} onChange={(e) => setDraft({ ...draft, port: Number(e.target.value) })} /></label>
                    </div>
                  </fieldset>
                  <fieldset>
                    <legend>Acceso y seguridad</legend>
                    <div className="database-field-row">
                      <label className="field"><span>Usuario</span><input required autoComplete="username" placeholder="leo_readonly" value={draft.username} onChange={(e) => setDraft({ ...draft, username: e.target.value })} /></label>
                      <label className="field"><span>Contraseña</span><input type="password" autoComplete="new-password" value={draft.password ?? ""} placeholder="Contraseña PostgreSQL" onChange={(e) => setDraft({ ...draft, password: e.target.value })} /></label>
                      <label className="field"><span>Modo SSL</span><select value={draft.ssl_mode} onChange={(e) => setDraft({ ...draft, ssl_mode: e.target.value })}><option value="disable">Desactivado</option><option value="prefer">Preferir</option><option value="require">Requerir</option><option value="verify-ca">Verificar CA</option><option value="verify-full">Verificación completa</option></select></label>
                    </div>
                  </fieldset>
                </div>
              ) : (
                <div className="database-fields">
                  <label className="field"><span>Nombre</span><input required value={draft.name} onChange={(e) => setDraft({ ...draft, name: e.target.value })} /></label>
                  <label className="field"><span>Host</span><input required placeholder="db.example.com" value={draft.host} onChange={(e) => setDraft({ ...draft, host: e.target.value })} /><small>Usa host.docker.internal para una base instalada en este host.</small></label>
                  <label className="field"><span>Puerto</span><input required type="number" min="1" max="65535" value={draft.port} onChange={(e) => setDraft({ ...draft, port: Number(e.target.value) })} /></label>
                  <label className="field"><span>Base de datos</span><input required value={draft.database} onChange={(e) => setDraft({ ...draft, database: e.target.value })} /></label>
                  <label className="field"><span>Usuario</span><input required autoComplete="username" value={draft.username} onChange={(e) => setDraft({ ...draft, username: e.target.value })} /></label>
                  <label className="field"><span>Contraseña</span><input type="password" autoComplete="new-password" value={draft.password ?? ""} placeholder={selected?.password_set ? "Dejar en blanco para conservar" : "Contraseña PostgreSQL"} onChange={(e) => setDraft({ ...draft, password: e.target.value })} /></label>
                  <label className="field"><span>Modo SSL</span><select value={draft.ssl_mode} onChange={(e) => setDraft({ ...draft, ssl_mode: e.target.value })}><option value="disable">Desactivado</option><option value="prefer">Preferir</option><option value="require">Requerir</option><option value="verify-ca">Verificar CA</option><option value="verify-full">Verificación completa</option></select></label>
                </div>
              )}
              {creating && <p className="database-create-safety"><strong>Usa acceso de solo lectura</strong><span>Limita este usuario a los esquemas y tablas que Leo necesite consultar.</span></p>}
              <label className="database-enabled"><input type="checkbox" checked={draft.enabled} onChange={(e) => setDraft({ ...draft, enabled: e.target.checked })} /><span>{creating ? <><strong>Activar al crear</strong><small>Leo podrá consultar esta conexión en cuanto esté guardada.</small></> : "Permitir que Leo use esta conexión"}</span></label>
              <div className="database-actions">
                <button type="submit" className="btn-primary" disabled={busy}>{creating ? busy ? "Creando…" : "Crear conexión" : "Guardar cambios"}</button>
                {creating && <button type="button" className="btn-ghost" disabled={busy} onClick={cancelCreate}>Cancelar</button>}
                {!creating && <button type="button" className="btn-secondary" disabled={busy} onClick={() => void test()}>Probar conexión</button>}
              </div>
              {result && <p className={result.ok ? "database-result ok" : "database-result bad"} role="status"><strong>{result.ok ? "Conexión correcta" : "Falló la conexión"}</strong><span>{result.detail}</span>{result.ok && !result.read_only && <span>Atención: este usuario no parece ser de solo lectura.</span>}</p>}
              {!creating && (confirmDelete ? <div className="confirm"><span>¿Eliminar {selected?.name}?</span><button type="button" className="btn-danger sm" disabled={busy} onClick={() => void remove()}>Eliminar</button><button type="button" className="btn-secondary sm" onClick={() => setConfirmDelete(false)}>Cancelar</button></div> : <button type="button" className="btn-danger database-delete" disabled={busy} onClick={() => setConfirmDelete(true)}>Eliminar conexión</button>)}
            </form>
          ) : <p className="catalog-empty">Selecciona una conexión o crea una nueva.</p>}
        </div>
      </div>
      {error && <p className="sheet-err" role="alert">{error}</p>}
    </aside>
  );
}
