import { useEffect, useLayoutEffect, useRef, useState, type FormEvent } from "react";
import { openExternal } from "./api";
import type { CodexLogin, Instruction, Op, Provider, Snapshot } from "./types";
import { Input, Select, TextArea } from "./components/Field";
import { Workspace } from "./components/Workspace";
import { Activity } from "./components/Activity";
import { Status } from "./components/Status";
import { IconPlus, IconCheck } from "./icons";
import { btn } from "./ui";
import type { Theme } from "./theme";

const KINDS = ["grok", "gpt", "ollama", "claude", "codex"] as const;
const KIND_LABEL: Record<string, string> = { grok: "xAI", gpt: "OpenAI", ollama: "Ollama", claude: "Anthropic", codex: "ChatGPT / Codex" };
const KEY_LABEL: Record<string, string> = { db: "Clave guardada", env: "Clave del entorno", falta: "Necesita configuración", none: "No requiere clave" };
type Props = {
  snap: Snapshot; onOp: (op: Op) => Promise<void>;
  onWebSearchChange: (enabled: boolean, contextSize: string) => Promise<void>;
  onInstruction: (input: { key: string; channel: string; content: string; active: boolean }) => Promise<void>;
  onInstructionReset: (key: string, channel: string) => Promise<void>;
  onCodexLogin: (providerId: string, onReady: (login: CodexLogin) => void) => Promise<void>;
  theme: Theme; onTheme: () => void; onClose: () => void;
};

export function Catalog({ snap, onOp, onWebSearchChange, onInstruction, onInstructionReset, onCodexLogin, theme, onTheme, onClose }: Props) {
  const [tab, setTab] = useState("providers");
  const contentRef = useRef<HTMLDivElement>(null);
  useLayoutEffect(() => { if (contentRef.current) contentRef.current.scrollTop = 0; }, [tab]);
  const [providerId, setProviderId] = useState<string | null>(null);
  const [creating, setCreating] = useState(false);
  const [name, setName] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  const provider = snap.providers.find((item) => item.id === providerId);
  const run = async (action: () => Promise<void>) => {
    setBusy(true); setError(""); setNotice("");
    try { await action(); setNotice("Cambio guardado."); }
    catch (e) { setError(e instanceof Error ? e.message : String(e)); }
    finally { setBusy(false); }
  };
  return <Workspace title="Ajustes" description="Configura cómo responde Ira y qué puede hacer." onClose={onClose}>
    <div className="settings-layout">
      <nav className="task-nav settings-nav" aria-label="Secciones de ajustes">{([["providers", "Proveedores"], ["behavior", "Comportamiento"], ["tools", "Herramientas"], ["appearance", "Apariencia"]] as const).map(([value, label]) => <button key={value} type="button" aria-pressed={tab === value} disabled={busy} onClick={() => { setTab(value); setError(""); setNotice(""); }}>{label}</button>)}</nav>
      <div ref={contentRef} className="settings-content">
        <div hidden={tab !== "providers"}>
          <header className="resource-heading"><div><h2>Proveedores</h2><p>Conecta una cuenta o configura una clave. El modelo se elige desde el chat.</p></div><button type="button" className={btn.secondary} disabled={busy || creating} onClick={() => { setCreating(true); setProviderId(null); }}><IconPlus />Añadir proveedor</button></header>
          <ul className="resource-list">{snap.providers.map((p) => <li key={p.id} className="resource-row"><div className="resource-top"><div className="resource-info"><strong>{p.name}</strong><p className="resource-description">{KIND_LABEL[p.kind] ?? p.kind} · {snap.models.filter((m) => m.provider_id === p.id).length} modelos</p><Status>{p.kind === "codex" ? p.key === "db" ? "Cuenta conectada" : "Necesita autorización" : KEY_LABEL[p.key]}</Status></div><button type="button" className={btn.secondary} disabled={busy} aria-expanded={providerId === p.id} onClick={() => { setProviderId(providerId === p.id ? null : p.id); setCreating(false); }}>{p.key === "falta" ? "Conectar" : "Configurar"}</button></div></li>)}</ul>
          {snap.providers.length === 0 && <div className="empty-state"><h3>Conecta tu primer proveedor</h3><p>Añade OpenAI, Anthropic, xAI, Ollama o una cuenta de ChatGPT para elegir un modelo.</p></div>}
          {creating && <form className="resource-editor" onSubmit={(e) => { e.preventDefault(); void run(async () => { await onOp({ op: "new_provider", name: name.trim() }); setCreating(false); setName(""); }); }}><h3>Nuevo proveedor</h3><Input label="Nombre del proveedor" autoFocus required value={name} disabled={busy} placeholder="Por ejemplo, OpenAI personal" onChange={(e) => setName(e.target.value)} /><p className="resource-description">Después de añadirlo, configura tipo de conexión y credenciales.</p><div className="form-actions"><button type="submit" className={btn.primary} disabled={busy || !name.trim()}>Añadir proveedor</button><button type="button" className={btn.ghost} disabled={busy} onClick={() => setCreating(false)}>Cancelar</button></div></form>}
          {provider && <ProviderEditor key={provider.id} provider={provider} onOp={onOp} onCodexLogin={onCodexLogin} canDelete={snap.providers.length > 1} onClose={() => setProviderId(null)} />}
        </div>
        <div hidden={tab !== "behavior"}>
          <header className="mb-6"><h2>Comportamiento</h2><p className="resource-description">Instrucciones compartidas entre modelos. Edita y guarda cada bloque.</p></header>
          {(snap.instructions ?? []).length > 0 ? snap.instructions?.map((block) => <InstructionField key={`${block.key}:${block.channel}`} block={block} onSave={onInstruction} onReset={onInstructionReset} />) : <>
            <PromptField label="Instrucciones del sistema" value={snap.system} onSave={(text) => onOp({ op: "set_system", text })} />
            <PromptField label="Instrucciones de voz" value={snap.voice_system} onSave={(text) => onOp({ op: "set_voice_system", text })} />
          </>}
          <section className="mt-8 border-t border-line pt-5"><h3 className="mb-3 font-semibold">Búsqueda web</h3>
            <label className="flex min-h-11 items-center gap-3 text-sm"><input type="checkbox" disabled={busy} checked={snap.web_search_enabled} onChange={(e) => void run(() => onWebSearchChange(e.target.checked, snap.web_search_context_size))} />Permitir búsqueda con proveedores compatibles</label>
            <Select label="Profundidad de búsqueda" disabled={busy || !snap.web_search_enabled} value={snap.web_search_context_size} onChange={(e) => void run(() => onWebSearchChange(snap.web_search_enabled, e.target.value))}><option value="low">Breve</option><option value="medium">Equilibrada</option><option value="high">Amplia</option></Select>
            <p className="resource-description">Disponibilidad según proveedor activo. Cambios guardados en la configuración local de Ira.</p>
          </section>
        </div>
        <div hidden={tab !== "tools"}>
          <header className="mb-6"><h2>Herramientas</h2><p className="resource-description">Controla acceso a herramientas y permiso para modificar datos.</p></header>
          <label className="flex min-h-14 items-center gap-3 border-b border-line py-3"><input type="checkbox" disabled={busy} checked={snap.tools_enabled} onChange={(e) => void run(() => onOp({ op: "set_tools_enabled", value: e.target.checked }))} /><span><strong className="block text-sm font-semibold">Permitir herramientas</strong><small className="text-muted">Ira puede consultar recursos conectados.</small></span></label>
          <label className="flex min-h-14 items-center gap-3 border-b border-line py-3"><input type="checkbox" disabled={busy || !snap.tools_enabled} checked={snap.tools_mutate} onChange={(e) => void run(() => onOp({ op: "set_tools_mutate", value: e.target.checked }))} /><span><strong className="block text-sm font-semibold">Permitir escritura</strong><small className="text-muted">Permite herramientas que modifican datos, archivos o ejecutan comandos.</small></span></label>
          <details className="advanced"><summary>{snap.tools.length} herramientas registradas</summary><ul className="advanced-body text-sm text-muted">{snap.tools.map((tool) => <li key={tool} className="wrap-anywhere">{tool}</li>)}</ul></details>
        </div>
        <div hidden={tab !== "appearance"}>
          <header className="mb-6"><h2>Apariencia</h2><p className="resource-description">Elige el tema que mejor se adapte a tu espacio.</p></header>
          <div className="resource-top"><div className="resource-info"><strong>Tema {theme === "dark" ? "oscuro" : "claro"}</strong><p className="resource-description">Se recuerda en este dispositivo.</p></div><button type="button" className={btn.secondary} onClick={onTheme}>Usar tema {theme === "dark" ? "claro" : "oscuro"}</button></div>
          <p className="resource-description mt-6">El movimiento respeta la preferencia de accesibilidad del sistema. Durante la lectura, las respuestas permanecen quietas.</p>
        </div>
        {busy && <p className="resource-result flex items-center gap-2 text-muted" role="status"><Activity />Guardando…</p>}
        {error && <p className="resource-result text-danger" role="alert">{error}</p>}
        {notice && <p className="resource-result text-listen" role="status">{notice}</p>}
      </div>
    </div>
  </Workspace>;
}

function ProviderEditor({ provider, onOp, onCodexLogin, canDelete, onClose }: {
  provider: Provider; onOp: Props["onOp"]; onCodexLogin: Props["onCodexLogin"]; canDelete: boolean; onClose: () => void;
}) {
  const [name, setName] = useState(provider.name);
  const [kind, setKind] = useState(provider.kind);
  const [url, setUrl] = useState(provider.base_url ?? "");
  const [key, setKey] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  const [login, setLogin] = useState<CodexLogin | null>(null);
  const [confirm, setConfirm] = useState(false);
  const isCodex = kind === "codex";
  const dirty = name !== provider.name || kind !== provider.kind || url !== (provider.base_url ?? "") || key.length > 0;
  const submit = async (event: FormEvent) => {
    event.preventDefault(); setBusy(true); setError(""); setNotice("");
    try {
      if (kind !== provider.kind) await onOp({ op: "set_kind", id: provider.id, kind });
      if (name.trim() !== provider.name) await onOp({ op: "rename_provider", id: provider.id, name: name.trim() });
      if (url !== (provider.base_url ?? "")) await onOp({ op: "set_base_url", id: provider.id, base_url: url });
      if (key && !isCodex) { await onOp({ op: "set_api_key", id: provider.id, api_key: key }); setKey(""); }
      setNotice("Configuración guardada. Credenciales guardadas no equivalen a acceso verificado.");
    } catch (e) { setError(`${e instanceof Error ? e.message : String(e)} Revisa los datos y reintenta; los cambios anteriores pueden haberse guardado.`); }
    finally { setBusy(false); }
  };
  const authorize = async () => {
    setBusy(true); setError(""); setNotice(""); setLogin(null);
    try { await onCodexLogin(provider.id, setLogin); setLogin(null); setNotice("Cuenta de ChatGPT conectada."); }
    catch (e) { setError(e instanceof Error ? e.message : String(e)); }
    finally { setBusy(false); }
  };
  return <section className="resource-editor" aria-label={`Configurar ${provider.name}`}>
    <h3>{provider.name}</h3>
    <form onSubmit={(e) => void submit(e)}>
      <fieldset disabled={busy}>
        <Input autoFocus label="Nombre del proveedor" required value={name} onChange={(e) => setName(e.target.value)} />
        <Select label="Tipo de conexión" value={kind} onChange={(e) => { setKind(e.target.value); setKey(""); setLogin(null); }}>
          {!KINDS.some((k) => k === kind) && <option value={kind}>{kind}</option>}{KINDS.map((k) => <option key={k} value={k}>{KIND_LABEL[k]}</option>)}
        </Select>
        {!isCodex && kind !== "ollama" && <Input label="Clave API" type="password" autoComplete="off" hint={`${KEY_LABEL[provider.key]}. Vacía conserva la clave actual.`} placeholder="Introduce una clave para guardarla" value={key} onChange={(e) => setKey(e.target.value)} />}
        {kind === "ollama" && <p className="resource-description mb-4">Ollama usa modelos instalados en tu equipo. Asegúrate de que su servidor esté iniciado.</p>}
        <details className="advanced"><summary>Avanzado · URL base</summary><div className="advanced-body"><Input label="URL base" type="url" placeholder="Predeterminada del proveedor" value={url} onChange={(e) => setUrl(e.target.value)} /></div></details>
      </fieldset>
      <div className="form-actions"><button type="submit" className={btn.primary} disabled={busy || !dirty || !name.trim()}>{busy ? <Activity /> : <IconCheck />}Guardar cambios</button><button type="button" className={btn.ghost} disabled={busy} onClick={onClose}>Cancelar</button></div>
    </form>
    {isCodex && <div className="mt-6 border-t border-line pt-5"><h4 className="font-semibold">Cuenta de ChatGPT</h4><p className="resource-description mb-3">Autoriza Ira en ChatGPT para usar Codex.</p><button type="button" className={btn.secondary} disabled={busy || kind !== provider.kind} onClick={() => void authorize()}>{busy && login ? "Esperando autorización…" : provider.key === "db" ? "Volver a conectar ChatGPT" : "Iniciar sesión con ChatGPT"}</button>{kind !== provider.kind && <p className="resource-description">Guarda el tipo de conexión antes de autorizar.</p>}
      {login && <div className="form-actions" role="status"><span>Código: <strong className="font-mono">{login.user_code}</strong></span><button type="button" className={btn.secondary} onClick={() => void openExternal(login.verification_url).catch((e) => setError(String(e)))}>Abrir ChatGPT</button></div>}
    </div>}
    {notice && <p className="resource-result text-listen" role="status">{notice}</p>}{error && <p className="resource-result text-danger" role="alert">{error}</p>}
    {canDelete && <details className="advanced"><summary>Eliminar proveedor</summary><div className="advanced-body"><p className="resource-description">También elimina sus modelos del catálogo.</p><div className="form-actions">{confirm ? <><button type="button" className={btn.danger} disabled={busy} onClick={() => {
      setBusy(true); void onOp({ op: "delete_provider", id: provider.id }).then(onClose).catch((e) => setError(String(e))).finally(() => setBusy(false));
    }}>Confirmar eliminación</button><button type="button" className={btn.ghost} disabled={busy} onClick={() => setConfirm(false)}>Cancelar</button></> : <button type="button" className={btn.danger} disabled={busy} onClick={() => setConfirm(true)}>Eliminar {provider.name}</button>}</div></div></details>}
  </section>;
}

const BLOCK_LABEL: Record<string, string> = { "persona:chat": "Personalidad y respuestas", "persona:voice": "Conversación por voz", "memory:chat": "Memoria", "tools:chat": "Uso de herramientas", "web_search:chat": "Búsqueda web" };
function InstructionField({ block, onSave, onReset }: { block: Instruction; onSave: Props["onInstruction"]; onReset: Props["onInstructionReset"] }) {
  const [content, setContent] = useState(block.content);
  const [active, setActive] = useState(block.active);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  const [reset, setReset] = useState(false);
  useEffect(() => { setContent(block.content); setActive(block.active); setReset(false); }, [block.content, block.active]);
  const dirty = content !== block.content || active !== block.active;
  const run = async (action: () => Promise<void>) => {
    setBusy(true); setError(""); setNotice("");
    try { await action(); setNotice("Instrucciones guardadas."); }
    catch (e) { setError(e instanceof Error ? e.message : String(e)); }
    finally { setBusy(false); }
  };
  return <form className="mb-6 border-b border-line pb-5" onSubmit={(e) => { e.preventDefault(); void run(() => onSave({ key: block.key, channel: block.channel, content, active })); }}>
    <TextArea label={BLOCK_LABEL[`${block.key}:${block.channel}`] ?? block.key} hint={block.channel === "voice" ? "Se aplica a voz." : "Se aplica a los modelos del canal correspondiente."} rows={block.key === "persona" ? 4 : 3} disabled={busy} value={content} onChange={(e) => setContent(e.target.value)} />
    <label className="flex items-center gap-2 text-sm"><input type="checkbox" checked={active} disabled={busy} onChange={(e) => setActive(e.target.checked)} />Instrucción activa</label>
    <div className="form-actions"><button type="submit" className={btn.secondary} disabled={busy || !dirty}>{busy ? <Activity /> : <IconCheck />}Guardar instrucciones</button><button type="button" className={btn.ghost} disabled={busy || !dirty} onClick={() => { setContent(block.content); setActive(block.active); setError(""); }}>Cancelar cambios</button>
      {reset ? <><span className="text-sm">¿Restaurar valores predeterminados?</span><button type="button" className={btn.danger} disabled={busy} onClick={() => void run(() => onReset(block.key, block.channel))}>Confirmar restauración</button><button type="button" className={btn.ghost} onClick={() => setReset(false)}>Conservar</button></> : <button type="button" className={btn.ghost} disabled={busy} onClick={() => setReset(true)}>Restaurar</button>}
    </div>{error && <p role="alert" className="resource-result text-danger">{error}</p>}{notice && <p role="status" className="resource-result text-listen">{notice}</p>}
  </form>;
}

function PromptField({ label, value, onSave }: { label: string; value: string; onSave: (text: string) => Promise<void> }) {
  const [draft, setDraft] = useState(value);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  return <form className="mb-6" onSubmit={(e) => { e.preventDefault(); setBusy(true); setError(""); setNotice(""); void onSave(draft).then(() => setNotice("Instrucciones guardadas.")).catch((e) => setError(String(e))).finally(() => setBusy(false)); }}>
    <TextArea label={label} value={draft} disabled={busy} rows={4} onChange={(e) => setDraft(e.target.value)} /><div className="form-actions"><button type="submit" className={btn.secondary} disabled={busy || draft === value}>Guardar instrucciones</button><button type="button" className={btn.ghost} disabled={busy || draft === value} onClick={() => setDraft(value)}>Cancelar cambios</button></div>{error && <p className="resource-result text-danger" role="alert">{error}</p>}{notice && <p className="resource-result text-listen" role="status">{notice}</p>}
  </form>;
}
