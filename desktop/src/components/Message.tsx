import { memo } from "react";
import { motion, useReducedMotion } from "motion/react";
import { Markdown } from "../Markdown";
import type { Bubble } from "../types";
import { Activity } from "./Activity";

export const Message = memo(function Message({ bubble, streaming }: { bubble: Bubble; streaming: boolean }) {
  const reduced = useReducedMotion();
  const user = bubble.kind === "user";
  return <motion.article className={`turn ${bubble.kind}`} role={bubble.kind === "error" ? "alert" : undefined} initial={reduced ? false : { y: user ? 16 : 8, scale: user ? 0.98 : 1 }}
    animate={{ y: 0, scale: 1 }} transition={{ type: "spring", stiffness: 420, damping: 34 }}>
    <div className="who">
      {bubble.kind === "ira" ? <img src="/ira-cabeza-recortada.png" alt="" /> : <span>{user ? "Tú" : "No se pudo completar"}</span>}
    </div>
    <div className={`bubble ${bubble.kind}`}>
      {bubble.kind === "ira" ? <><Markdown text={bubble.text} />{streaming && <Activity receiving />}</> : <p>{bubble.text}</p>}
    </div>
  </motion.article>;
});
