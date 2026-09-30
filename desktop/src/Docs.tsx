/**
 * Documentation sheet. Hosts the standalone docs app (docs-app/, built into
 * public/docs) inside a sheet so the rail can reach technical documentation
 * without leaving the shell. The framed app is independent: its own bundle,
 * language, and theme state.
 */
import { motion } from "motion/react";
import { IconClose } from "./icons";
import { useSheetFocus } from "./components/useSheetFocus";
import { btn, docsFrame, docsHead, sheetDocs } from "./ui";

type Props = { onClose: () => void };

export function DocsSheet({ onClose }: Props) {
  const sheetRef = useSheetFocus();
  return (
    <motion.aside
      ref={sheetRef}
      role="dialog"
      aria-modal="true"
      tabIndex={-1}
      className={sheetDocs}
      aria-label="Documentación"
      initial={{ x: "100%" }}
      animate={{ x: 0 }}
      exit={{ x: "100%" }}
      transition={{ type: "spring", stiffness: 360, damping: 38 }}
    >
      <header className={docsHead}>
        <div>
          <h2>Documentación</h2>
          <p>Documentación técnica de Ira. Idioma y tema propios.</p>
        </div>
        <button type="button" className={btn.icon} aria-label="Cerrar documentación" title="Cerrar" onClick={onClose}>
          <IconClose />
        </button>
      </header>
      <iframe
        className={docsFrame}
        title="Documentación técnica de Ira"
        src="/docs/index.html"
      />
    </motion.aside>
  );
}
