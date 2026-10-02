import { useEffect, useRef, type ReactNode } from "react";
import { motion } from "motion/react";
import { IconClose } from "../icons";
import { btn } from "../ui";

/** A task surface alongside navigation, not a modal over the conversation. */
export function Workspace({ title, description, onClose, children }: {
  title: string; description: string; onClose: () => void; children: ReactNode;
}) {
  const closeRef = useRef<HTMLButtonElement>(null);
  useEffect(() => {
    const previous = document.activeElement as HTMLElement | null;
    closeRef.current?.focus();
    return () => {
      const candidates = [previous, ...document.querySelectorAll<HTMLElement>('[data-mobile-menu="true"], [data-desktop-menu="true"]')];
      candidates.find((el) => el?.isConnected && el.getClientRects().length && !el.closest('[inert]') && getComputedStyle(el).visibility !== "hidden")?.focus();
    };
  }, []);
  return <motion.section className="workspace" aria-label={title}
    initial={{ opacity: 0, x: 10 }} animate={{ opacity: 1, x: 0 }}
    transition={{ duration: 0.2, ease: [0.16, 1, 0.3, 1] }}>
    <header className="workspace-header">
      <div><h1>{title}</h1><p>{description}</p></div>
      <button ref={closeRef} type="button" className={btn.secondary} onClick={onClose} aria-label={`Cerrar ${title.toLowerCase()}`}><span>Volver al chat</span><IconClose /></button>
    </header>
    <div className="workspace-body">{children}</div>
  </motion.section>;
}
