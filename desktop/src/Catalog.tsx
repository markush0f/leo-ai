/**
 * Catalog editor. Local state holds form drafts; `onOp` persists changes and the
 * parent supplies the refreshed snapshot. Key fields accept replacements without
 * exposing stored secrets.
 */
import { useState } from "react";
import { motion } from "motion/react";
import { openExternal } from "./api";
import { type CodexLogin, type Model, type Op, type Provider, type Snapshot } from "./types";
import { Input, Select, TextArea } from "./components/Field";
import { useSheetFocus } from "./components/useSheetFocus";
import { IconClose, IconSearch } from "./icons";
import { btn, catalogCurrent, catalogDetail, catalogEmpty, catalogLayout, catalogProvider, catalogProviderOn, catalogProviders, confirm, cx, dot, dotOn, effortCatalog, modelPick, sectionHead, settings, settingsBody, sheet, sheetErr, sheetHead } from "./ui";

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
  onWebSearchChange: (enabled: boolean, contextSize: string) => Promise<void>;
  onCodexLogin: (
    providerId: string,
    onReady: (login: CodexLogin) => void,
  ) => Promise<void>;
  onClose: () => void;
};

export function Catalog({ snap, onOp, onWebSearchChange, onCodexLogin, onClose }: Props) {
  const sheetRef = useSheetFocus();
  const active = snap.providers.find((p) =>
    snap.models.some((m) => m.id === snap.active_model_id && m.provider_id === p.id),
  );
  const [openId, setOpenId] = useState<string | null>(active?.id ?? snap.providers[0]?.id ?? null);
  const [system, setSystem] = useState(snap.system);
  const [webSearchEnabled, setWebSearchEnabled] = useState(snap.web_search_enabled);
  const [webSearchContextSize, setWebSearchContextSize] = useState(snap.web_search_context_size);
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
    <motion.aside
      ref={sheetRef} role="dialog" aria-modal="true" tabIndex={-1}
      className={sheet}
      aria-label="Catálogo"
      initial={{ x: "100%" }}
      animate={{ x: 0 }}
      exit={{ x: "100%" }}
      transition={{ type: "spring", stiffness: 360, damping: 38 }}
    >
      <header className={sheetHead}>
        <div>
          <h2>Modelos y configuración</h2>
          <p>Elige el modelo con el que quieres conversar.</p>
        </div>
        <button type="button" className={btn.icon} aria-label="Cerrar catálogo" title="Cerrar" onClick={onClose}>
          <IconClose />
        </button>
      </header>

      <div className={catalogCurrent}>
        <span className={cx(dot, activeModel && dotOn)} aria-hidden />
        <div className="min-w-0 phone:flex-1"><span className="text-[0.8rem] text-muted">Modelo en uso</span><strong>{activeModel?.display_name ?? "Ningún modelo seleccionado"}</strong></div>
        {active && <span className="ml-auto text-[0.85rem] wrap-anywhere text-muted phone:ml-5 phone:w-full">{active.name}</span>}
      </div>

      <div className={catalogLayout}>
      <nav aria-label="Proveedores del catálogo">
      <h3 className="flex justify-between px-3 text-[0.9rem] font-semibold">Proveedores <span className="text-[0.8rem] font-[450] text-muted tabular-nums">{snap.providers.length}</span></h3>
      <ul className={catalogProviders}>
        {snap.providers.map((p) => {
          const isActive =
            snap.models.find((m) => m.id === snap.active_model_id)?.provider_id === p.id;
          return (
            <li key={p.id}>
              <button
                type="button"
                className={cx(catalogProvider, open?.id === p.id && catalogProviderOn)}
                aria-pressed={open?.id === p.id}
                aria-controls="catalog-provider-detail"
                onClick={() => setOpenId(p.id)}
              >
                <span className={cx("font-semibold", open?.id === p.id && "text-accent")}>{p.name}</span>
                <span className="text-[0.8rem] text-muted">{snap.models.filter((m) => m.provider_id === p.id).length} modelos{isActive ? " · En uso" : ""}</span>
              </button>
            </li>
          );
        })}
      </ul>
      </nav>

      <div id="catalog-provider-detail" className={catalogDetail}>
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
      ) : <p className={catalogEmpty}>No hay proveedores configurados.</p>}
      </div>
      </div>

      {err && <p className={sheetErr} role="alert">{err}</p>}
      <details className={cx(settings, "mt-8")}>
        <summary>Instrucciones de Ira <span>Para todos los modelos</span></summary>
        <div className={settingsBody}>
            <TextArea label="Instrucciones del sistema" hint="Se guardan al salir del campo." value={system} rows={4} onChange={(e) => setSystem(e.target.value)}
              onBlur={() => {
                if (system !== snap.system) void run({ op: "set_system", text: system });
              }} />
          <div className="grid gap-3 border-t border-line pt-4">
            <label className="flex items-center gap-3 text-sm">
              <input
                type="checkbox"
                checked={webSearchEnabled}
                onChange={(event) => {
                  const enabled = event.target.checked;
                  setWebSearchEnabled(enabled);
                  void onWebSearchChange(enabled, webSearchContextSize).catch((error) => {
                    setWebSearchEnabled(!enabled);
                    setErr(error instanceof Error ? error.message : String(error));
                  });
                }}
              />
              Permitir búsqueda web con proveedores compatibles
            </label>
            <label className="grid gap-1 text-sm">
              Profundidad de búsqueda
              <select
                className="w-full rounded-[11px] border border-line bg-bg px-3 py-2 text-ink"
                value={webSearchContextSize}
                onChange={(event) => {
                  const contextSize = event.target.value;
                  setWebSearchContextSize(contextSize);
                  void onWebSearchChange(webSearchEnabled, contextSize).catch((error) => {
                    setErr(error instanceof Error ? error.message : String(error));
                  });
                }}
              >
                <option value="low">Breve</option>
                <option value="medium">Equilibrada</option>
                <option value="high">Amplia</option>
              </select>
            </label>
            <p className="m-0 text-xs text-muted">Se guarda en ~/.ira/config.toml. La disponibilidad depende del proveedor activo.</p>
          </div>
          {snap.tools.length > 0 && <div><h4 className="m-0 text-[0.9rem] font-semibold">Herramientas disponibles</h4><ul className="mt-2 flex list-none flex-wrap gap-x-4 gap-y-2 p-0 text-[0.85rem] wrap-anywhere text-muted">{snap.tools.map((tool) => <li key={tool}>{tool}</li>)}</ul></div>}
        </div>
      </details>
    </motion.aside>
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
  const filteredModels = models.filter((model) => `${model.display_name} ${model.name}`.toLowerCase().includes(query.trim().toLowerCase()));

  return (
    <div aria-busy={busy}>
      <header className="[&_h3]:m-0 [&_h3]:text-[1.3rem] [&_h3]:leading-[1.3] [&_h3]:tracking-[-0.02em] [&_h3]:wrap-anywhere [&_p]:mt-[0.4rem] [&_p]:text-[0.9rem] [&_p]:text-muted [&_p_span]:mx-1">
        <h3>{provider.name}</h3>
        <p>{KIND_LABEL[provider.kind] ?? provider.kind} <span>·</span> {isCodex ? (provider.key === "db" ? "Cuenta conectada" : "Sin conectar") : KEY_LABEL[provider.key]}</p>
      </header>
      <section className="mt-6" aria-label="Modelos disponibles">
        <div className={sectionHead}><h4>Modelos.dev</h4><span>{models.length} disponibles</span></div>
        {models.length > 0 && <Input label="Buscar modelo" icon={<IconSearch />} type="search" placeholder="Buscar por nombre…" value={query} onChange={(e) => setQuery(e.target.value)} />}
        <ul className="m-0 list-none p-0 [&_li]:flex [&_li]:items-center [&_li]:gap-1 [&_li]:border-b [&_li]:border-line">
          {filteredModels.map((m) => (
            <li key={m.id} className={m.id === activeModelId ? "rounded-lg border-b-transparent bg-[color-mix(in_srgb,var(--color-accent)_8%,var(--color-surface))]" : undefined}>
              <button type="button" className={cx(modelPick, m.id === activeModelId && "disabled:cursor-default")} disabled={busy || m.id === activeModelId} onClick={() => void onOp({ op: "activate_model", id: m.id })} aria-label={`${m.id === activeModelId ? "Modelo en uso:" : "Usar modelo"} ${m.display_name}`}>
                <span className="min-w-0 flex-1"><strong className="block wrap-anywhere text-[0.95rem] font-[550]">{m.display_name}</strong><small className="block wrap-anywhere text-[0.72rem] text-muted">{m.name}{m.context_window ? ` · ${(m.context_window / 1000).toLocaleString()}k contexto` : ""}</small></span>
                <span className={cx("shrink-0 text-[0.8rem] text-muted", m.id === activeModelId && "font-[650] text-accent")}>{m.id === activeModelId ? "En uso" : "Usar"}</span>
              </button>
              {m.effort_options.length > 0 ? (
                <select className={effortCatalog} aria-label={`Potencia de ${m.display_name}`} disabled={busy || m.id !== activeModelId} value={m.effort || m.effort_options[0]} onChange={(event) => void onOp({ op: "set_model_effort", id: m.id, effort: event.target.value })}>
                  {m.effort_options.map((effort) => <option key={effort} value={effort}>{effort}</option>)}
                </select>
              ) : <span className="px-3 text-xs text-muted">{m.reasoning ? "Razonamiento" : "Potencia fija"}</span>}
            </li>
          ))}
        </ul>
        {filteredModels.length === 0 && <div className={catalogEmpty}><p>{models.length === 0 ? "Este proveedor todavía no tiene modelos." : "No hay modelos con ese nombre."}</p>{query && <button type="button" className={btn.secondary} onClick={() => setQuery("")}>Limpiar búsqueda</button>}</div>}
      </section>

      <details className={cx(settings, "mt-6")}>
      <summary>Configuración del proveedor <span>Conexión y credenciales</span></summary>
      <div className={settingsBody}>
      <p className="mb-4 text-[0.8rem] text-muted">Los cambios se guardan al salir de cada campo.</p>
      <div className="mb-[0.85rem] flex gap-[0.45rem]">
        <button type="button" className={btn.secondary} disabled={busy} onClick={() => void onOp({ op: "activate_provider", id: provider.id })}>
          Activar proveedor
        </button>
      </div>
        <Input label="Nombre del proveedor" disabled={busy}
          value={name}
          onChange={(e) => setName(e.target.value)}
          onBlur={() => {
            if (name.trim() && name !== provider.name) {
              void onOp({ op: "rename_provider", id: provider.id, name: name.trim() });
            }
          }}
        />
        <Select label="Tipo de conexión" value={provider.kind} disabled={busy} onChange={(e) => void onOp({ op: "set_kind", id: provider.id, kind: e.target.value })}>
          {!KINDS.some((kind) => kind === provider.kind) && <option value={provider.kind}>{provider.kind}</option>}
          {KINDS.map((kind) => <option key={kind} value={kind}>{KIND_LABEL[kind]}</option>)}
        </Select>

      {isCodex ? (
        <section className="mb-[0.9rem] grid gap-[0.65rem] rounded-xl border border-line bg-elevated p-[0.8rem]" aria-live="polite">
          <div className="flex items-center gap-[0.55rem]">
            <span className="font-[650]">ChatGPT Plus</span>
            <span className={cx("ml-auto text-[0.78rem] text-muted", provider.key === "db" && "text-listen")}>
              {provider.key === "db" ? "conectado" : "sin conectar"}
            </span>
          </div>
          <p className="m-0 text-[0.82rem] text-muted">Inicia sesión con ChatGPT para usar Codex. Ira nunca muestra tus tokens.</p>
          <button
            type="button"
            className={btn.primary}
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
            <div className="flex flex-wrap items-center gap-[0.55rem] pt-[0.2rem]">
              <span className="text-[0.78rem] text-muted">Código</span>
              <strong className="tracking-[0.12em]">{login.user_code}</strong>
              <button
                type="button"
                className={cx(btn.secondary, "ml-auto")}
                onClick={() => void openExternal(login.verification_url)}
              >
                Abrir ChatGPT
              </button>
            </div>
          )}
          {authErr && <p className={sheetErr}>{authErr}</p>}
        </section>
      ) : (
          <Input label={`Clave API · ${KEY_LABEL[provider.key]}`} disabled={busy}
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
      )}

        <Input label="URL base" disabled={busy}
          value={url}
          placeholder="URL predeterminada del proveedor"
          onChange={(e) => setUrl(e.target.value)}
          onBlur={() => {
            if (url !== (provider.base_url ?? "")) {
              void onOp({ op: "set_base_url", id: provider.id, base_url: url });
            }
          }}
        />

      {canDelete &&
        (pendingDelete === "provider" ? (
          <p className={confirm}>
            ¿borrar {provider.name}?
            <button
              type="button"
              className={cx(btn.danger, btn.sm)}
              disabled={busy}
              onClick={() => void onOp({ op: "delete_provider", id: provider.id })}
            >
              Borrar
            </button>
            <button type="button" className={cx(btn.secondary, btn.sm)} onClick={() => setPendingDelete(null)}>
              Cancelar
            </button>
          </p>
        ) : (
          <button
            type="button"
            className={btn.danger}
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
