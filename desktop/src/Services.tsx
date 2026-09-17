/**
 * Host service status (Postgres + MCP Toolbox) and a single start action.
 */
import { IconPower } from "./icons";
import type { Services } from "./types";

type Props = {
  data: Services | null;
  starting: boolean;
  compact?: boolean;
  onStart: () => void;
};

export function ServiceBoard({ data, starting, compact, onStart }: Props) {
  const items = data?.services ?? [
    { id: "postgres", name: "Postgres", running: false, healthy: false, detail: "…" },
    { id: "toolbox", name: "Toolbox", running: false, healthy: false, detail: "…" },
  ];
  const ready = Boolean(data?.ok);
  return (
    <section className={`svc${compact ? " compact" : ""}`} aria-label="Servicios">
      <p className="svc-label">Servicios</p>
      <ul className="svc-list">
        {items.map((s) => (
          <li key={s.id}>
            <span className={s.healthy ? "dot on" : "dot"} aria-hidden />
            <span className="svc-name">{s.name}</span>
            <span className="svc-detail">{s.detail}</span>
          </li>
        ))}
      </ul>
      {data?.error ? <p className="svc-err">{data.error}</p> : null}
      <button
        type="button"
        className="btn-primary svc-start"
        disabled={starting || ready}
        onClick={onStart}
      >
        <IconPower />
        {starting ? "Arrancando…" : ready ? "En marcha" : "Arrancar servicios"}
      </button>
    </section>
  );
}
