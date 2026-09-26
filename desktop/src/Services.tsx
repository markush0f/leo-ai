/**
 * Host service status. Each Compose service can be started or stopped.
 */
import type { Services } from "./types";

type Props = {
  data: Services | null;
  busyId: string | null;
  compact?: boolean;
  onToggle: (id: string, running: boolean) => void;
};

const FALLBACK = [
  { id: "postgres", name: "Postgres", running: false, healthy: false, detail: "…" },
  { id: "toolbox", name: "Toolbox", running: false, healthy: false, detail: "…" },
  { id: "ira-realtime", name: "Realtime", running: false, healthy: false, detail: "…" },
  { id: "colibri", name: "Colibrì", running: false, healthy: false, detail: "…" },
  { id: "veritas-kanban", name: "Veritas", running: false, healthy: false, detail: "…" },
  { id: "veritas-mcp", name: "Veritas MCP", running: false, healthy: false, detail: "…" },
];

export function ServiceBoard({ data, busyId, compact, onToggle }: Props) {
  const items = data?.services.length ? data.services : FALLBACK;
  return (
    <section className={`svc${compact ? " compact" : ""}`} aria-label="Servicios">
      <p className="svc-label">Servicios</p>
      <ul className="svc-list">
        {items.map((s) => {
          const busy = busyId === s.id;
          const label = busy ? "…" : s.running ? "Parar" : "Arrancar";
          return (
            <li key={s.id} title={`${s.name}: ${s.detail}`}>
              <span className={s.healthy ? "dot on" : "dot"} aria-hidden />
              <span className="svc-name">{s.name}</span>
              <span className="svc-detail">{s.detail}</span>
              <button
                type="button"
                className="btn-ghost svc-toggle"
                disabled={busyId !== null}
                aria-label={`${label} ${s.name}`}
                onClick={() => onToggle(s.id, s.running)}
              >
                {label}
              </button>
            </li>
          );
        })}
      </ul>
      {data?.error ? <p className="svc-err">{data.error}</p> : null}
    </section>
  );
}
