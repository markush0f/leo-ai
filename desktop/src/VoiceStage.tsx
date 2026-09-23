import { useEffect, useRef, type MutableRefObject } from "react";
import { motion } from "motion/react";
import { IconClose } from "./icons";

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

  useEffect(() => {
    let frame = 0;
    const tick = () => {
      orbRef.current?.style.setProperty("--voice-level", String(levelRef.current));
      frame = requestAnimationFrame(tick);
    };
    frame = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(frame);
  }, [levelRef]);

  return (
    <>
      <motion.button type="button" className="scrim voice" aria-label="colgar" initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }} onClick={onHangup} />
      <motion.div
        className="voice-card"
        role="dialog"
        aria-modal="true"
        aria-labelledby="voice-status"
        initial={{ opacity: 0, scale: 0.985 }}
        animate={{ opacity: 1, scale: 1 }}
        exit={{ opacity: 0, scale: 0.99 }}
        transition={{ duration: 0.18 }}
      >
        <button type="button" className="btn-ghost voice-close" aria-label="colgar" onClick={onHangup}>
          <IconClose />
        </button>
        <div className="voice-halo" data-phase={phase}>
          <div ref={orbRef} className="voice-orb" data-phase={phase} />
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
      </motion.div>
    </>
  );
}
