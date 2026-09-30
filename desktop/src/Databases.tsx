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
import { btn, catalogDetail, catalogEmpty, catalogLayout, catalogLayoutEmpty, catalogNav, catalogProvider, catalogProviderOn, catalogProviders, confirm, cx, sheet, sheetErr, sheetHead } from "./ui";
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
      className={sheet}
      aria-label="Bases de datos"
      aria-busy={busy || loading}
      initial={{ x: "100%" }}
      animate={{ x: 0 }}
      exit={{ x: "100%" }}
      transition={{ type: "spring", stiffness: 360, damping: 38 }}
    >
      <header className={sheetHead}>
        <div>
          <h2>Bases de datos</h2>
          <p>Conexiones PostgreSQL disponibles para Ira.</p>
        </div>
        <button type="button" className={btn.icon} aria-label="Cerrar bases de datos" title="Cerrar" onClick={onClose}><IconClose /></button>
      </header>

      <div className={cx("grid gap-[0.15rem] rounded-[10px] border border-[color-mix(in_srgb,var(--color-accent)_45%,var(--color-line))] bg-[color-mix(in_srgb,var(--color-accent)_8%,var(--color-bg))] p-[0.85rem] px-4 text-[0.85rem] [&_span]:text-muted", creating && "hidden")}>
        <strong>Usa un usuario PostgreSQL de solo lectura.</strong>
        <span>Concede acceso únicamente a los esquemas y tablas necesarios.</span>
      </div>

      <div className={cx(catalogLayout, creating && items.length === 0 && catalogLayoutEmpty, creating && items.length === 0 && "empty-connections")}>
        <nav className={catalogNav} aria-label="Conexiones PostgreSQL">
          <div className="flex items-center gap-2 [&_h3]:flex-1">
            <h3 className="text-[0.9rem] font-semibold">Conexiones</h3>
            <button type="button" className={cx(btn.secondary, btn.sm)} disabled={busy || loading} onClick={beginCreate}><IconPlus />Nueva</button>
          </div>
          <ul className={catalogProviders}>
            {items.map((item) => (
              <li key={item.id}>
                <button type="button" disabled={busy} aria-pressed={!creating && selectedId === item.id} className={cx(catalogProvider, !creating && selectedId === item.id && catalogProviderOn)} onClick={() => choose(item)}>
                  <span className={cx("font-semibold", !creating && selectedId === item.id && "text-accent")}>{item.name}</span>
                  <span className="text-[0.8rem] text-muted">{item.host}:{item.port} · {connectionState(item)}</span>
                </button>
              </li>
            ))}
          </ul>
          {loading && <p className={catalogEmpty} role="status">Cargando conexiones…</p>}
          {!loading && items.length === 0 && <p className={catalogEmpty}>Tu primera conexión aparecerá aquí.</p>}
        </nav>

        <div className={catalogDetail}>
          {(creating || selected) ? (
            <form className={cx("grid gap-4", creating && "gap-[1.4rem]")} onSubmit={(event) => void submit(event)} onChange={() => setResult(null)}
              onInvalid={(event) => { const advanced = (event.target as HTMLElement).closest("details"); if (advanced) advanced.open = true; }}>
              {creating ? (
                <header className="flex items-start gap-4 pb-3 [&_h3]:m-0 [&_h3]:text-[1.35rem] [&_h3]:leading-[1.2] [&_h3]:tracking-[-0.025em] [&_p]:mt-1 [&_p]:mb-[0.7rem] [&_p]:text-[0.9rem] [&_p]:text-muted [&_code]:block [&_code]:max-w-full [&_code]:font-[inherit] [&_code]:text-[0.82rem] [&_code]:text-ink [&_code]:tabular-nums [&_code]:wrap-anywhere">
                  <span className="grid w-8 basis-8 place-items-center text-accent [&_svg]:size-7"><IconDatabase /></span>
                  <div>
                    <h3>Conecta PostgreSQL</h3>
                    <p>Indica dónde está tu base de datos y quién puede leerla.</p>
                    <code>{draft.host || "servidor"}:{draft.port}/{draft.database || "base_de_datos"}</code>
                  </div>
                </header>
              ) : (
                <header className="[&_h3]:m-0 [&_h3]:text-[1.3rem] [&_h3]:leading-[1.3] [&_h3]:tracking-[-0.02em] [&_h3]:wrap-anywhere [&_p]:mt-[0.4rem] [&_p]:text-[0.9rem] [&_p]:text-muted">
                  <h3>{selected?.name}</h3>
                    <p>{selected ? connectionState(selected) : ""} · {selected?.password_set ? "Contraseña guardada" : "Sin contraseña guardada"}</p>
                </header>
              )}
              {selected?.enabled && selected.last_test_ok !== true && selected.last_test_error && <p className="m-0 grid gap-[0.15rem] rounded-lg border border-danger p-3 text-[0.85rem] wrap-anywhere [&_span]:text-muted" role="alert">{selected.last_test_error}</p>}
              <fieldset className="m-0 min-w-0 border-0 p-0" disabled={busy}>
                {creating && <section className="border-b border-line pb-6" aria-label="Importar conexión">
                  <Input label="¿Tienes una URL de conexión?" type="password" icon={<IconLink />} autoComplete="off"
                    placeholder="postgresql://usuario:contraseña@host/base" value={connectionUrl}
                    error={urlError} hint="Pégala para completar los campos. También puedes rellenarlos abajo."
                    onChange={(event) => { setConnectionUrl(event.target.value); setUrlError(undefined); setImported(false); }}
                    onKeyDown={(event) => { if (event.key === "Enter") { event.preventDefault(); importUrl(); } }} />
                  <button type="button" className={btn.secondary} disabled={!connectionUrl.trim()} onClick={importUrl}><IconLink />Completar desde URL</button>
                  {imported && <p className="mt-3 flex items-center gap-2 text-[0.8rem] text-listen [&_svg]:size-[18px]" role="status"><IconCheck />Datos completados. Puedes revisarlos abajo.</p>}
                </section>}
                <section className="mt-6">
                  <h4 className="mb-[0.9rem] text-base font-semibold">Destino</h4>
                  <Input label="Nombre de la conexión" required placeholder="Mi base de datos" value={draft.name} onChange={(event) => setDraft({ ...draft, name: event.target.value })} />
                  <div className="grid grid-cols-2 gap-[0.8rem] phone:grid-cols-1">
                    <Input label="Servidor" required placeholder="db.example.com" value={draft.host} onChange={(event) => setDraft({ ...draft, host: event.target.value })} />
                    <Input label="Base de datos" required placeholder="ira" value={draft.database} onChange={(event) => setDraft({ ...draft, database: event.target.value })} />
                  </div>
                  <p className="m-0 text-[0.78rem] leading-[1.5] text-muted">Desde Docker, usa host.docker.internal para acceder a una base instalada en tu equipo.</p>
                </section>
                <section className="mt-6">
                  <h4 className="mb-[0.9rem] text-base font-semibold">Credenciales</h4>
                  <div className="grid grid-cols-2 gap-[0.8rem] phone:grid-cols-1">
                    <Input label="Usuario" required autoComplete="username" placeholder="ira_readonly" value={draft.username} onChange={(event) => setDraft({ ...draft, username: event.target.value })} />
                    <Input label="Contraseña" type="password" autoComplete="new-password" value={draft.password ?? ""} placeholder={selected?.password_set ? "Conservar la guardada" : "Contraseña PostgreSQL"} onChange={(event) => setDraft({ ...draft, password: event.target.value })} />
                  </div>
                </section>
                <details className="group mt-6 border-y border-line">
                  <summary className="cursor-pointer py-[0.9rem] text-[0.9rem] font-semibold group-open:mb-2">Opciones avanzadas <span className="mt-[0.2rem] block text-[0.78rem] font-[450] text-muted">Puerto {draft.port} · SSL {draft.ssl_mode}</span></summary>
                  <div className="grid grid-cols-2 gap-[0.8rem] phone:grid-cols-1">
                    <Input label="Puerto" required type="number" min="1" max="65535" value={draft.port || ""} onChange={(event) => setDraft({ ...draft, port: Number(event.target.value) })} />
                    <Select label="Modo SSL" value={draft.ssl_mode} onChange={(event) => setDraft({ ...draft, ssl_mode: event.target.value })}>
                      <option value="disable">Desactivado</option><option value="prefer">Preferir</option><option value="require">Requerir</option><option value="verify-ca">Verificar CA</option><option value="verify-full">Verificación completa</option>
                    </Select>
                  </div>
                </details>
              </fieldset>
              {creating && <p className="m-0 grid gap-[0.15rem] rounded-[10px] border border-[color-mix(in_srgb,var(--color-accent)_40%,var(--color-line))] p-[0.8rem] px-4 text-[0.85rem] [&_span]:text-muted"><strong>Usa acceso de solo lectura</strong><span>Limita este usuario a los esquemas y tablas que Ira necesite consultar.</span></p>}
              <label className={cx("flex items-center gap-[0.65rem] font-[550] [&_input]:h-[1.15rem] [&_input]:w-8 [&_input]:accent-accent [&_span]:grid [&_span]:gap-[0.1rem] [&_strong]:text-[0.9rem] [&_small]:text-[0.8rem] [&_small]:font-[450] [&_small]:text-muted", creating && "rounded-[10px] bg-[color-mix(in_srgb,var(--color-accent)_7%,var(--color-bg))] p-[0.9rem] px-4")}><input type="checkbox" disabled={busy} checked={draft.enabled} onChange={(e) => setDraft({ ...draft, enabled: e.target.checked })} /><span><strong>Disponible para Ira</strong><small>Permitir que las herramientas consulten esta conexión.</small></span></label>
              <div className="sticky -bottom-8 z-[1] m-0 flex flex-wrap items-center gap-2 border-t border-line bg-surface py-4 phone:-bottom-6">
                <button type="submit" className={cx(btn.primary, creating && "min-w-40")} disabled={busy}>{busy ? <Activity /> : <IconCheck />}{phase === "saving" ? "Guardando…" : phase === "testing" ? "Comprobando…" : "Guardar y comprobar"}</button>
                {creating && <button type="button" className={btn.ghost} disabled={busy} onClick={cancelCreate}>Cancelar</button>}
                <span className="text-[0.78rem] text-muted" role="status">{phase === "testing" ? "Guardada. Probando acceso a PostgreSQL." : ""}</span>
              </div>
              {result && <p className={cx("m-0 grid gap-[0.15rem] rounded-lg border p-3 text-[0.85rem] wrap-anywhere [&_span]:text-muted", result.ok ? "border-listen" : "border-danger")} role="status"><strong>{result.ok ? "Conexión correcta" : "Falló la conexión"}</strong><span>{result.detail}</span>{result.ok && !result.read_only && <span>Atención: este usuario no parece ser de solo lectura.</span>}</p>}
              {!creating && (confirmDelete ? <div className={confirm}><span>¿Eliminar {selected?.name}?</span><button type="button" className={cx(btn.danger, btn.sm)} disabled={busy} onClick={() => void remove()}>Eliminar</button><button type="button" className={cx(btn.secondary, btn.sm)} onClick={() => setConfirmDelete(false)}>Cancelar</button></div> : <button type="button" className={cx(btn.danger, "mt-2 justify-self-start")} disabled={busy} onClick={() => setConfirmDelete(true)}>Eliminar conexión</button>)}
            </form>
          ) : <p className={catalogEmpty}>Selecciona una conexión o crea una nueva.</p>}
        </div>
      </div>
      {error && <p className={sheetErr} role="alert">{error}</p>}
    </motion.aside>
  );
}
