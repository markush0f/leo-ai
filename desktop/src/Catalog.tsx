/**
 * Catalog editor. Local state holds form drafts; `onOp` persists changes and the
 * parent supplies the refreshed snapshot. Key fields accept replacements without
 * exposing stored secrets.
 */
import { useState } from "react";
import type { Model, Op, Provider, Snapshot } from "./types";

const KINDS = ["grok", "gpt", "ollama", "claude", "codex"] as const;

const KEY_LABEL: Record<string, string> = {
  db: "en bbdd",
  env: "env",
  falta: "falta",
  none: "—",
};

type Props = {
  snap: Snapshot;
  onOp: (op: Op) => Promise<void>;
  onClose: () => void;
};

export function Catalog({ snap, onOp, onClose }: Props) {
  const active = snap.providers.find((p) =>
    snap.models.some((m) => m.id === snap.active_model_id && m.provider_id === p.id),
  );
  const [openId, setOpenId] = useState<string | null>(active?.id ?? snap.providers[0]?.id ?? null);
  const [system, setSystem] = useState(snap.system);
  const [newProvider, setNewProvider] = useState("");
  const [newModel, setNewModel] = useState("");
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

  const open = snap.providers.find((p) => p.id === openId);

  return (
    <aside className="sheet" aria-label="Catálogo">
      <header className="sheet-head">
        <h2>Catálogo</h2>
        <button type="button" className="btn-ghost" onClick={onClose}>
          Cerrar
        </button>
      </header>

      <label className="field">
        <span>system prompt</span>
        <textarea
          value={system}
          rows={4}
          onChange={(e) => setSystem(e.target.value)}
          onBlur={() => {
            if (system !== snap.system) void run({ op: "set_system", text: system });
          }}
        />
      </label>

      {snap.tools.length > 0 && (
        <p className="tools">tools: {snap.tools.join(", ")}</p>
      )}

      <ul className="providers">
        {snap.providers.map((p) => {
          const isActive =
            snap.models.find((m) => m.id === snap.active_model_id)?.provider_id === p.id;
          return (
            <li key={p.id}>
              <button
                type="button"
                className={openId === p.id ? "row on" : "row"}
                onClick={() => setOpenId(p.id)}
              >
                <span className={isActive ? "dot on" : "dot"} aria-hidden />
                <span className="row-name">{p.name}</span>
                <span className="row-meta">{p.kind}</span>
              </button>
            </li>
          );
        })}
      </ul>

      {open && (
        <ProviderEditor
          key={open.id}
          provider={open}
          models={snap.models.filter((m) => m.provider_id === open.id)}
          activeModelId={snap.active_model_id}
          canDelete={snap.providers.length > 1}
          newModel={newModel}
          setNewModel={setNewModel}
          busy={busy}
          onOp={run}
        />
      )}

      <form
        className="add"
        onSubmit={(e) => {
          e.preventDefault();
          const name = newProvider.trim();
          if (!name) return;
          void run({ op: "new_provider", name }).then(() => setNewProvider(""));
        }}
      >
        <input
          value={newProvider}
          onChange={(e) => setNewProvider(e.target.value)}
          placeholder="nuevo proveedor"
          aria-label="nuevo proveedor"
        />
        <button type="submit" className="btn-primary" disabled={busy || !newProvider.trim()}>
          Añadir
        </button>
      </form>

      {err && <p className="sheet-err">{err}</p>}
    </aside>
  );
}

function ProviderEditor({
  provider,
  models,
  activeModelId,
  canDelete,
  newModel,
  setNewModel,
  busy,
  onOp,
}: {
  provider: Provider;
  models: Model[];
  activeModelId: string | null;
  canDelete: boolean;
  newModel: string;
  setNewModel: (v: string) => void;
  busy: boolean;
  onOp: (op: Op) => Promise<void>;
}) {
  const [name, setName] = useState(provider.name);
  const [url, setUrl] = useState(provider.base_url ?? "");
  const [key, setKey] = useState("");
  const [pendingDelete, setPendingDelete] = useState<"provider" | string | null>(null);
  const isCodex = provider.kind === "codex";

  const nextKind = KINDS[(KINDS.indexOf(provider.kind as (typeof KINDS)[number]) + 1) % KINDS.length];

  return (
    <div className="editor">
      <label className="field">
        <span>nombre</span>
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

      <div className="pair">
        <button
          type="button"
          className="btn-secondary"
          disabled={busy}
          onClick={() => void onOp({ op: "set_kind", id: provider.id, kind: nextKind })}
        >
          {provider.kind}
        </button>
        <button
          type="button"
          className="btn-primary"
          disabled={busy}
          onClick={() => void onOp({ op: "activate_provider", id: provider.id })}
        >
          Activar
        </button>
      </div>

      <label className="field">
        <span>{isCodex ? "credenciales OAuth" : "api key"} · {KEY_LABEL[provider.key]}</span>
        <input
          type="password"
          autoComplete="off"
          value={key}
          placeholder={isCodex ? "pegar JSON OAuth para guardar" : "escribir para guardar"}
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
      {isCodex && (
        <p className="field-help">
          Usa la sesión OAuth de ChatGPT. No introduzcas una API key de OpenAI.
        </p>
      )}

      <label className="field">
        <span>base url</span>
        <input
          value={url}
          onChange={(e) => setUrl(e.target.value)}
          onBlur={() => {
            if (url !== (provider.base_url ?? "")) {
              void onOp({ op: "set_base_url", id: provider.id, base_url: url });
            }
          }}
        />
      </label>

      <ul className="models">
        {models.map((m) => (
          <li key={m.id} className={m.id === activeModelId ? "on" : undefined}>
            {pendingDelete === m.id ? (
              <span className="confirm">
                ¿borrar {m.name}?
                <button
                  type="button"
                  className="btn-danger sm"
                  onClick={() => void onOp({ op: "delete_model", id: m.id })}
                >
                  Sí
                </button>
                <button type="button" className="btn-secondary sm" onClick={() => setPendingDelete(null)}>
                  No
                </button>
              </span>
            ) : (
              <>
                <span className="model-name">{m.name}</span>
                <button
                  type="button"
                  className={m.id === activeModelId ? "model-active" : "btn-primary sm"}
                  disabled={busy || m.id === activeModelId}
                  onClick={() => void onOp({ op: "activate_model", id: m.id })}
                >
                  {m.id === activeModelId ? "Activo" : "Activar"}
                </button>
                <button
                  type="button"
                  className="btn-danger sm"
                  onClick={() => setPendingDelete(m.id)}
                >
                  Borrar
                </button>
              </>
            )}
          </li>
        ))}
      </ul>

      <form
        className="add"
        onSubmit={(e) => {
          e.preventDefault();
          const n = newModel.trim();
          if (!n) return;
          void onOp({ op: "new_model", provider_id: provider.id, name: n }).then(() =>
            setNewModel(""),
          );
        }}
      >
        <input
          value={newModel}
          onChange={(e) => setNewModel(e.target.value)}
          placeholder="nuevo modelo"
          aria-label="nuevo modelo"
        />
        <button type="submit" className="btn-primary" disabled={busy || !newModel.trim()}>
          Añadir
        </button>
      </form>

      {canDelete &&
        (pendingDelete === "provider" ? (
          <p className="confirm">
            ¿borrar {provider.name}?
            <button
              type="button"
              className="btn-danger sm"
              onClick={() => void onOp({ op: "delete_provider", id: provider.id })}
            >
              Sí
            </button>
            <button type="button" className="btn-secondary sm" onClick={() => setPendingDelete(null)}>
              No
            </button>
          </p>
        ) : (
          <button
            type="button"
            className="btn-danger"
            onClick={() => setPendingDelete("provider")}
          >
            Borrar proveedor
          </button>
        ))}
    </div>
  );
}
