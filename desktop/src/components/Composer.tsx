import { useLayoutEffect, useRef, type RefObject } from "react";
import { motion } from "motion/react";
import { IconMic, IconSend, IconTools } from "../icons";
import { EFFORTS, type Snapshot } from "../types";

type Props = {
  input: string; onInput: (value: string) => void; onSend: () => void; onTalk: () => void;
  snap: Snapshot | null; busy: boolean; listening: boolean; canTalk: boolean; canSend: boolean;
  boxRef: RefObject<HTMLTextAreaElement | null>;
  onModel: (id: string) => void; onEffort: (id: string, effort: string) => void; onTools: () => void;
};

export function Composer({ input, onInput, onSend, onTalk, snap, busy, listening, canTalk, canSend, boxRef, onModel, onEffort, onTools }: Props) {
  const composing = useRef(false);
  useLayoutEffect(() => {
    const box = boxRef.current;
    if (!box) return;
    box.style.height = "auto";
    box.style.height = `${Math.min(box.scrollHeight, 176)}px`;
  }, [input, boxRef]);
  const active = snap?.models.find((model) => model.id === snap.active_model_id);
  return <motion.form className={`composer${busy ? " is-working" : ""}`} onSubmit={(event) => { event.preventDefault(); onSend(); }}>
    <textarea ref={boxRef} value={input} rows={1} disabled={!snap || listening}
      placeholder={listening ? "Te escucho…" : snap ? "¿Qué hacemos hoy?" : "Conecta el catálogo para empezar"}
      aria-label="Mensaje para Ira" onChange={(event) => onInput(event.target.value)}
      onCompositionStart={() => { composing.current = true; }} onCompositionEnd={() => { composing.current = false; }}
      onKeyDown={(event) => {
        if (event.key === "Enter" && !event.shiftKey && !event.nativeEvent.isComposing && !composing.current) {
          event.preventDefault(); onSend();
        }
      }} />
    <div className="composer-bar">
      <select className="model-select" aria-label="Modelo activo" disabled={!snap || busy || listening}
        value={snap?.active_model_id ?? ""} onChange={(event) => onModel(event.target.value)}>
        {!active && <option value="">Seleccionar modelo</option>}
        {snap?.providers.map((provider) => <optgroup key={provider.id} label={provider.name}>
          {snap.models.filter((model) => model.provider_id === provider.id).map((model) => <option key={model.id} value={model.id}>{model.name}</option>)}
        </optgroup>)}
      </select>
      <select className="effort-select" aria-label="Tipo de razonamiento" disabled={!active || busy || listening}
        value={active?.effort || "low"} onChange={(event) => active && onEffort(active.id, event.target.value)}>
        {EFFORTS.map((effort) => <option key={effort} value={effort}>{effort}</option>)}
      </select>
      <button type="button" className={`mode-chip${snap?.tools_enabled ? " on" : ""}`} disabled={!snap || busy}
        aria-pressed={snap?.tools_enabled ?? false} title="Permitir herramientas" onClick={onTools}>
        <IconTools /><span>Herramientas</span>
      </button>
      <span className="composer-grow" />
      <button type="button" className={`btn-mic${listening ? " on" : ""}`} disabled={!canTalk}
        data-sheet-trigger={listening ? "true" : undefined}
        aria-pressed={listening} aria-label={listening ? "Dejar de hablar" : "Hablar con Ira"} title="Hablar con Ira" onClick={onTalk}><IconMic /></button>
      <motion.button type="submit" className="btn-send" disabled={!canSend} aria-label="Enviar mensaje" title="Enviar mensaje"
        whileTap={{ scale: 0.88 }}><IconSend /></motion.button>
    </div>
  </motion.form>;
}
