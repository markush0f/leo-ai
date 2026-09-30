import { useLayoutEffect, useRef, type RefObject } from "react";
import { motion } from "motion/react";
import { IconMic, IconSend, IconTools } from "../icons";
import type { Snapshot } from "../types";
import { btn, chip, chipOn, composer, composerBar, composerBox, cx, effortSelect, modelSelect } from "../ui";

type Props = {
  input: string; onInput: (value: string) => void; onSend: () => void; onTalk: () => void;
  snap: Snapshot | null; busy: boolean; listening: boolean; canTalk: boolean; canSend: boolean;
  boxRef: RefObject<HTMLTextAreaElement | null>;
  onModel: (id: string) => void; onEffort: (id: string, effort: string) => void; onTools: () => void; onMutate: () => void;
};

export function Composer({ input, onInput, onSend, onTalk, snap, busy, listening, canTalk, canSend, boxRef, onModel, onEffort, onTools, onMutate }: Props) {
  const composing = useRef(false);
  useLayoutEffect(() => {
    const box = boxRef.current;
    if (!box) return;
    box.style.height = "auto";
    box.style.height = `${Math.min(box.scrollHeight, 176)}px`;
  }, [input, boxRef]);
  const active = snap?.models.find((model) => model.id === snap.active_model_id);
  return <motion.form className={composer} onSubmit={(event) => { event.preventDefault(); onSend(); }}>
    <textarea ref={boxRef} className={composerBox} value={input} rows={1} disabled={!snap || listening}
      placeholder={listening ? "Te escucho…" : snap ? "¿Qué hacemos hoy?" : "Conecta el catálogo para empezar"}
      aria-label="Mensaje para Ira" onChange={(event) => onInput(event.target.value)}
      onCompositionStart={() => { composing.current = true; }} onCompositionEnd={() => { composing.current = false; }}
      onKeyDown={(event) => {
        if (event.key === "Enter" && !event.shiftKey && !event.nativeEvent.isComposing && !composing.current) {
          event.preventDefault(); onSend();
        }
      }} />
    <div className={composerBar}>
      <select className={modelSelect} aria-label="Modelo activo" disabled={!snap || busy || listening}
        value={snap?.active_model_id ?? ""} onChange={(event) => onModel(event.target.value)}>
        {!active && <option value="">Seleccionar modelo</option>}
        {snap?.providers.map((provider) => <optgroup key={provider.id} label={provider.name}>
          {snap.models.filter((model) => model.provider_id === provider.id).map((model) => <option key={model.id} value={model.id}>{model.display_name}</option>)}
        </optgroup>)}
      </select>
      {active?.effort_options.length ? <select className={effortSelect} aria-label="Potencia del modelo" disabled={!active || busy || listening}
        value={active?.effort || active?.effort_options[0]} onChange={(event) => active && onEffort(active.id, event.target.value)}>
        {active.effort_options.map((effort) => <option key={effort} value={effort}>{effort}</option>)}
      </select> : <span className="text-xs text-muted">{active?.reasoning ? "Razonamiento" : "Potencia fija"}</span>}
      <button type="button" className={cx(chip, snap?.tools_enabled && chipOn)} disabled={!snap || busy}
        aria-pressed={snap?.tools_enabled ?? false} title="Permitir herramientas" onClick={onTools}>
        <IconTools /><span>Herramientas</span>
      </button>
      <button type="button" className={cx(chip, snap?.tools_mutate && chipOn)} disabled={!snap || busy || !snap.tools_enabled}
        aria-pressed={snap?.tools_mutate ?? false} title="Al activarla, Ira puede usar herramientas que escriben o borran archivos, ejecutan comandos y modifican datos. Desactivada, solo usa herramientas marcadas como lectura." onClick={onMutate}>
        <span>Escritura</span>
      </button>
      <span className="flex-1 phone:hidden" />
      <button type="button" className={cx(btn.mic, listening && btn.micOn)} disabled={!canTalk}
        data-sheet-trigger={listening ? "true" : undefined}
        aria-pressed={listening} aria-label={listening ? "Dejar de hablar" : "Hablar con Ira"} title="Hablar con Ira" onClick={onTalk}><IconMic /></button>
      <motion.button type="submit" className={btn.send} disabled={!canSend} aria-label="Enviar mensaje" title="Enviar mensaje"
        whileTap={{ scale: 0.88 }}><IconSend /></motion.button>
    </div>
  </motion.form>;
}
