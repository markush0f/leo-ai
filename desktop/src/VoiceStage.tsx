import { useEffect, useRef, type MutableRefObject } from "react";
import { motion, useReducedMotion } from "motion/react";
import { IconClose } from "./icons";
import { useSheetFocus } from "./components/useSheetFocus";

export type VoicePhase = "connect" | "listen" | "wait" | "speak" | "error";

type Props = {
  phase: VoicePhase;
  userText: string;
  iraText: string;
  error: string | null;
  levelRef: MutableRefObject<number>;
  onHangup: () => void;
};

const STATUS: Record<VoicePhase, string> = {
  connect: "Conectando…",
  listen: "Te escucho",
  wait: "Ira piensa",
  speak: "Ira habla",
  error: "No pude hablar",
};

export function VoiceStage({ phase, userText, iraText, error, levelRef, onHangup }: Props) {
  const orbRef = useRef<HTMLDivElement>(null);
  const sheetRef = useSheetFocus();
  const reducedMotion = useReducedMotion();

  useEffect(() => {
    if (reducedMotion) return;
    let frame = 0;
    const tick = () => {
      orbRef.current?.style.setProperty("--voice-level", String(levelRef.current));
      frame = requestAnimationFrame(tick);
    };
    frame = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(frame);
  }, [levelRef, reducedMotion]);

  return (
    <>
      <motion.button type="button" className="scrim voice" aria-label="colgar" initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }} onClick={onHangup} />
      <motion.section ref={sheetRef} tabIndex={-1}
        className="voice-card"
        role="dialog"
        aria-modal="true"
        aria-labelledby="voice-status"
        initial={{ y: 24, scale: 0.94 }}
        animate={{ y: 0, scale: 1 }}
        exit={{ y: 16, scale: 0.94, opacity: 0 }}
        transition={{ type: "spring", stiffness: 350, damping: 30 }}
      >
        <button type="button" className="btn-ghost voice-close" aria-label="colgar" onClick={onHangup}>
          <IconClose />
        </button>
        <div className="voice-halo" data-phase={phase}>
          <div ref={orbRef} className="voice-orb" data-phase={phase}><img src="/ira-cabeza-recortada.png" alt="" /></div>
        </div>
        <p id="voice-status" className="voice-status" aria-live="polite">
          {error ?? STATUS[phase]}
        </p>
        {userText ? <p className="voice-you">{userText}</p> : null}
        {iraText ? (
          <p className="voice-ira" aria-live="polite">
            {iraText}
          </p>
        ) : null}
        <button type="button" className="btn-danger voice-hangup" autoFocus onClick={onHangup}>
          Colgar
        </button>
      </motion.section>
    </>
  );
}
