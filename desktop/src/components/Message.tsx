import { memo } from "react";
import { motion, useReducedMotion } from "motion/react";
import { Markdown } from "../Markdown";
import type { Bubble } from "../types";
import { Activity } from "./Activity";
import { IconTools } from "../icons";
import { bubbleError, bubbleIra, bubbleUser, cx, turnError, turnIra, turnUser, whoError, whoIra, whoUser } from "../ui";

export const Message = memo(function Message({ bubble, streaming }: { bubble: Bubble; streaming: boolean }) {
  const reduced = useReducedMotion();
  const user = bubble.kind === "user";
  const ira = bubble.kind === "ira";
  return <motion.article className={ira ? turnIra : user ? turnUser : turnError} role={bubble.kind === "error" ? "alert" : undefined} initial={reduced ? false : { y: user ? 16 : 8, scale: user ? 0.98 : 1 }}
    animate={{ y: 0, scale: 1 }} transition={{ duration: 0.2, ease: [0.16, 1, 0.3, 1] }}>
    <div className={ira ? whoIra : user ? whoUser : whoError}>
      {bubble.kind === "ira" ? <img src="/ira-cabeza-recortada.png" alt="" /> : <span>{user ? "Tú" : "No se pudo completar"}</span>}
    </div>
    <div className={cx(ira ? bubbleIra : user ? bubbleUser : bubbleError)}>
      {bubble.kind === "ira" ? <>{bubble.mcps && bubble.mcps.length > 0 && <details className="tool-trace"><summary><IconTools />{bubble.mcps.length} {bubble.mcps.length === 1 ? "herramienta utilizada" : "herramientas utilizadas"}</summary><ul>{bubble.mcps.map((tool) => <li key={tool}>{tool}</li>)}</ul></details>}<Markdown text={bubble.text} />{streaming && <span className="mt-2 flex items-center gap-2 text-xs text-muted"><Activity receiving />Recibiendo respuesta</span>}</> : <p>{bubble.text}</p>}
      {bubble.notes?.map((note) => <p key={`${note.action}:${note.text}`} className="m-0 mt-2 text-xs text-muted">{note.action === "forgotten" ? "Olvidado" : "Guardado"}: {note.text}</p>)}
    </div>
  </motion.article>;
});
