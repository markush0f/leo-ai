/**
 * Catalog editor. Local state holds form drafts; `onOp` persists changes and the
 * parent supplies the refreshed snapshot. Key fields accept replacements without
 * exposing stored secrets.
 */
import { useState } from "react";
import { openExternal } from "./api";
import type { CodexLogin, Model, Op, Provider, Snapshot } from "./types";

const KINDS = ["grok", "gpt", "ollama", "claude", "codex"] as const;

const KEY_LABEL: Record<string, string> = {
  db: "Clave guardada",
  env: "Configurada en el entorno",
  falta: "Sin configurar",
  none: "No requiere clave",
};

const KIND_LABEL: Record<string, string> = {
  grok: "xAI", gpt: "OpenAI", ollama: "Ollama", claude: "Anthropic", codex: "ChatGPT / Codex",
};

type Props = {
  snap: Snapshot;
  onOp: (op: Op) => Promise<void>;
  onCodexLogin: (
    providerId: string,
    onReady: (login: CodexLogin) => void,
  ) => Promise<void>;
  onClose: () => void;
};

export function Catalog({ snap, onOp, onCodexLogin, onClose }: Props) {
  const active = snap.providers.find((p) =>
    snap.models.some((m) => m.id === snap.active_model_id && m.provider_id === p.id),
  );
  const [openId, setOpenId] = useState<string | null>(active?.id ?? snap.providers[0]?.id ?? null);
  const [system, setSystem] = useState(snap.system);
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState<string | null>(null);

  const run = async (op: Op) => {
    setBusy(true);
    setErr(null);
    try {
      await onOp(op);
    } catch (e) {
      setErr(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  };

  const open = snap.providers.find((p) => p.id === openId) ?? active ?? snap.providers[0];
  const activeModel = snap.models.find((m) => m.id === snap.active_model_id);

  return (
    <aside className="sheet catalog" aria-label="Catálogo">
      <header className="sheet-head">
        <div>
          <h2>Catálogo</h2>
          <p>Elige el modelo con el que quieres conversar.</p>
        </div>
        <button type="button" className="btn-ghost" onClick={onClose}>
          Cerrar
        </button>
      </header>

      <div className="catalog-current">
        <span className={activeModel ? "dot on" : "dot"} aria-hidden />
        <div><span>Modelo en uso</span><strong>{activeModel?.name ?? "Ningún modelo seleccionado"}</strong></div>
        {active && <span className="catalog-current-provider">{active.name}</span>}
      </div>

      <div className="catalog-layout">
      <nav className="catalog-nav" aria-label="Proveedores del catálogo">
      <h3>Proveedores <span>{snap.providers.length}</span></h3>
      <ul className="catalog-providers">
        {snap.providers.map((p) => {
          const isActive =
            snap.models.find((m) => m.id === snap.active_model_id)?.provider_id === p.id;
          return (
            <li key={p.id}>
              <button
                type="button"
                className={open?.id === p.id ? "catalog-provider selected" : "catalog-provider"}
                aria-pressed={open?.id === p.id}
                aria-controls="catalog-provider-detail"
                onClick={() => setOpenId(p.id)}
              >
                <span className="catalog-provider-name">{p.name}</span>
                <span className="catalog-provider-meta">{snap.models.filter((m) => m.provider_id === p.id).length} modelos{isActive ? " · En uso" : ""}</span>
              </button>
            </li>
          );
        })}
      </ul>
      </nav>

      <div id="catalog-provider-detail" className="catalog-detail">
      {open ? (
        <ProviderEditor
          key={open.id}
          provider={open}
          models={snap.models.filter((m) => m.provider_id === open.id)}
          activeModelId={snap.active_model_id}
          canDelete={snap.providers.length > 1}
          busy={busy}
          onOp={run}
          onCodexLogin={onCodexLogin}
        />
      ) : <p className="catalog-empty">No hay proveedores configurados.</p>}
      </div>
      </div>

      {err && <p className="sheet-err" role="alert">{err}</p>}
      <details className="catalog-settings catalog-global">
        <summary>Instrucciones de Ira <span>Para todos los modelos</span></summary>
        <div className="catalog-settings-body">
          <label className="field">
            <span>Instrucciones del sistema</span>
            <textarea value={system} rows={4} onChange={(e) => setSystem(e.target.value)}
              onBlur={() => {
                if (system !== snap.system) void run({ op: "set_system", text: system });
              }} />
          </label>
          <p className="field-help">Se guardan al salir del campo.</p>
          {snap.tools.length > 0 && <div className="catalog-tools"><h4>Herramientas disponibles</h4><ul>{snap.tools.map((tool) => <li key={tool}>{tool}</li>)}</ul></div>}
        </div>
      </details>
    </aside>
  );
}

function ProviderEditor({
  provider,
  models,
  activeModelId,
  canDelete,
  busy,
  onOp,
  onCodexLogin,
}: {
  provider: Provider;
  models: Model[];
  activeModelId: string | null;
  canDelete: boolean;
  busy: boolean;
  onOp: (op: Op) => Promise<void>;
  onCodexLogin: (
    providerId: string,
    onReady: (login: CodexLogin) => void,
  ) => Promise<void>;
}) {
  const [name, setName] = useState(provider.name);
  const [url, setUrl] = useState(provider.base_url ?? "");
  const [key, setKey] = useState("");
  const [pendingDelete, setPendingDelete] = useState<"provider" | string | null>(null);
  const [login, setLogin] = useState<CodexLogin | null>(null);
  const [authBusy, setAuthBusy] = useState(false);
  const [authErr, setAuthErr] = useState<string | null>(null);
  const [query, setQuery] = useState("");
  const isCodex = provider.kind === "codex";
  const filteredModels = models.filter((model) => model.name.toLowerCase().includes(query.trim().toLowerCase()));

  return (
    <div className="catalog-editor" aria-busy={busy}>
      <header className="catalog-detail-head">
        <h3>{provider.name}</h3>
        <p>{KIND_LABEL[provider.kind] ?? provider.kind} <span>·</span> {isCodex ? (provider.key === "db" ? "Cuenta conectada" : "Sin conectar") : KEY_LABEL[provider.key]}</p>
      </header>
      <section className="catalog-model-section" aria-label="Modelos disponibles">
        <div className="catalog-section-head"><h4>Modelos</h4><span>{models.length} disponibles</span></div>
        {models.length > 0 && <label className="field catalog-search"><span>Buscar modelo</span><input type="search" placeholder="Buscar por nombre…" value={query} onChange={(e) => setQuery(e.target.value)} /></label>}
        <ul className="catalog-models">
          {filteredModels.map((m) => (
            <li key={m.id} className={m.id === activeModelId ? "active" : undefined}>
              {pendingDelete === m.id ? (
                <div className="confirm">
                  <span>¿Borrar {m.name}?</span>
                  <button type="button" className="btn-danger sm" disabled={busy} onClick={() => void onOp({ op: "delete_model", id: m.id })}>Borrar</button>
                  <button type="button" className="btn-secondary sm" onClick={() => setPendingDelete(null)}>Cancelar</button>
                </div>
              ) : (
                <>
                  <button type="button" className="catalog-model-pick" disabled={busy || m.id === activeModelId} onClick={() => void onOp({ op: "activate_model", id: m.id })} aria-label={`${m.id === activeModelId ? "Modelo en uso:" : "Usar modelo"} ${m.name}`}>
                    <span className="catalog-model-name">{m.name}</span>
                    <span className="catalog-model-action">{m.id === activeModelId ? "En uso" : "Usar"}</span>
                  </button>
                  <button type="button" className="catalog-remove" disabled={busy} aria-label={`Borrar modelo ${m.name}`} title={`Borrar ${m.name}`} onClick={() => setPendingDelete(m.id)}>
                    <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true"><path d="M4 7h16M9 7V4h6v3M6 7l1 13h10l1-13M10 11v5M14 11v5" /></svg>
                  </button>
                </>
              )}
            </li>
          ))}
        </ul>
        {filteredModels.length === 0 && <div className="catalog-empty"><p>{models.length === 0 ? "Este proveedor todavía no tiene modelos." : "No hay modelos con ese nombre."}</p>{query && <button type="button" className="btn-secondary" onClick={() => setQuery("")}>Limpiar búsqueda</button>}</div>}
      </section>

      <details className="catalog-settings">
      <summary>Configuración del proveedor <span>Conexión y credenciales</span></summary>
      <div className="catalog-settings-body">
      <p className="catalog-help">Los cambios se guardan al salir de cada campo.</p>
      <div className="pair">
        <button type="button" className="btn-secondary" disabled={busy} onClick={() => void onOp({ op: "activate_provider", id: provider.id })}>
          Activar proveedor
        </button>
      </div>
      <label className="field">
        <span>Nombre del proveedor</span>
        <input
          value={name}
          onChange={(e) => setName(e.target.value)}
          onBlur={() => {
            if (name.trim() && name !== provider.name) {
              void onOp({ op: "rename_provider", id: provider.id, name: name.trim() });
            }
          }}
        />
      </label>

      <label className="field">
        <span>Tipo de conexión</span>
        <select value={provider.kind} disabled={busy} onChange={(e) => void onOp({ op: "set_kind", id: provider.id, kind: e.target.value })}>
          {!KINDS.some((kind) => kind === provider.kind) && <option value={provider.kind}>{provider.kind}</option>}
          {KINDS.map((kind) => <option key={kind} value={kind}>{KIND_LABEL[kind]}</option>)}
        </select>
      </label>

      {isCodex ? (
        <section className="codex-auth" aria-live="polite">
          <div className="codex-auth-head">
            <span>ChatGPT Plus</span>
            <span className={provider.key === "db" ? "auth-status on" : "auth-status"}>
              {provider.key === "db" ? "conectado" : "sin conectar"}
            </span>
          </div>
          <p>Inicia sesión con ChatGPT para usar Codex. Ira nunca muestra tus tokens.</p>
          <button
            type="button"
            className="btn-primary"
            disabled={busy || authBusy}
            onClick={() => {
              setAuthBusy(true);
              setAuthErr(null);
              setLogin(null);
              void onCodexLogin(provider.id, setLogin)
                .catch((e) => setAuthErr(e instanceof Error ? e.message : String(e)))
                .finally(() => setAuthBusy(false));
            }}
          >
            {authBusy ? "Esperando autorización…" : provider.key === "db" ? "Volver a conectar" : "Iniciar sesión con ChatGPT"}
          </button>
          {login && (
            <div className="codex-code">
              <span>Código</span>
              <strong>{login.user_code}</strong>
              <button
                type="button"
                className="btn-secondary"
                onClick={() => void openExternal(login.verification_url)}
              >
                Abrir ChatGPT
              </button>
            </div>
          )}
          {authErr && <p className="sheet-err">{authErr}</p>}
        </section>
      ) : (
        <label className="field">
          <span>Clave API · {KEY_LABEL[provider.key]}</span>
          <input
            type="password"
            autoComplete="off"
            value={key}
            placeholder="Introduce una clave para guardarla"
            onChange={(e) => setKey(e.target.value)}
            onBlur={() => {
              if (key.length > 0) {
                void onOp({ op: "set_api_key", id: provider.id, api_key: key }).then(() =>
                  setKey(""),
                );
              }
            }}
          />
        </label>
      )}

      <label className="field">
        <span>URL base</span>
        <input
          value={url}
          placeholder="URL predeterminada del proveedor"
          onChange={(e) => setUrl(e.target.value)}
          onBlur={() => {
            if (url !== (provider.base_url ?? "")) {
              void onOp({ op: "set_base_url", id: provider.id, base_url: url });
            }
          }}
        />
      </label>

      {canDelete &&
        (pendingDelete === "provider" ? (
          <p className="confirm">
            ¿borrar {provider.name}?
            <button
              type="button"
              className="btn-danger sm"
              disabled={busy}
              onClick={() => void onOp({ op: "delete_provider", id: provider.id })}
            >
              Borrar
            </button>
            <button type="button" className="btn-secondary sm" onClick={() => setPendingDelete(null)}>
              Cancelar
            </button>
          </p>
        ) : (
          <button
            type="button"
            className="btn-danger"
            disabled={busy}
            onClick={() => setPendingDelete("provider")}
          >
            Borrar proveedor
          </button>
        ))}
      </div>
      </details>
    </div>
  );
}
