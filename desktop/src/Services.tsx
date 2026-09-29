/**
 * Host service status. Each Compose service can be started or stopped.
 */
import { useState } from "react";
import { motion } from "motion/react";
import { IconClose } from "./icons";
import { useSheetFocus } from "./components/useSheetFocus";
import type { Service, Services } from "./types";

type Props = {
  data: Services | null;
  busyId: string | null;
  compact?: boolean;
  onToggle: (id: string, running: boolean) => void;
};

const FALLBACK: Service[] = [
  { id: "postgres", name: "Postgres", running: false, healthy: false, detail: "…" },
  { id: "toolbox", name: "Toolbox", running: false, healthy: false, detail: "…" },
  { id: "ira-realtime", name: "Realtime", running: false, healthy: false, detail: "…" },
  { id: "colibri", name: "Colibrì", running: false, healthy: false, detail: "…" },
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

type SheetProps = {
  data: Services | null;
  busyId: string | null;
  onClose: () => void;
  onToggle: (id: string, running: boolean) => void;
  onBoot: (id: string, on: boolean) => void;
  onPort: (id: string, port: number) => void;
  onMeta: (id: string, name: string, description: string) => void;
  onStartSelected: () => void;
};

export function ServicesSheet({ data, busyId, onClose, onToggle, onBoot, onPort, onMeta, onStartSelected }: SheetProps) {
  const sheetRef = useSheetFocus();
  const items = data?.services.length ? data.services : FALLBACK;
  const services = items.filter((item) => item.kind !== "mcp");
  const mcps = items.filter((item) => item.kind === "mcp");
  const [gateway, setGateway] = useState(String(data?.gateway_port ?? 8790));
  const [ports, setPorts] = useState<Record<string, string>>({});
  return (
    <motion.aside
      ref={sheetRef}
      role="dialog"
      aria-modal="true"
      tabIndex={-1}
      className="sheet catalog services-sheet"
      aria-label="Servicios"
      initial={{ x: "100%" }}
      animate={{ x: 0 }}
      exit={{ x: "100%" }}
      transition={{ type: "spring", stiffness: 360, damping: 38 }}
    >
      <header className="sheet-head">
        <div>
          <h2>Servicios</h2>
          <p>El servicio local y su MCP van juntos. El nombre y la descripción le dicen a Ira cuál usar.</p>
        </div>
        <button type="button" className="btn-ghost icon-button" aria-label="Cerrar servicios" title="Cerrar" onClick={onClose}>
          <IconClose />
        </button>
      </header>
      <form className="svc-port" onSubmit={(event) => {
        event.preventDefault();
        const port = Number(gateway);
        if (port >= 1 && port <= 65535) onPort("ira-gateway", port);
      }}>
        <label htmlFor="gateway-port">Puerto expuesto del gateway</label>
        <input id="gateway-port" inputMode="numeric" value={gateway} onChange={(event) => setGateway(event.target.value)} />
        <button type="submit" className="btn-ghost" disabled={busyId !== null}>Aplicar</button>
      </form>
      <p className="svc-note">Marca qué sube en cada arranque. Un par local arranca y para junto.</p>
      <ServiceGroup title="Servicios" items={services} all={items} busyId={busyId} ports={ports} setPorts={setPorts} onToggle={onToggle} onBoot={onBoot} onPort={onPort} onMeta={onMeta} />
      <ServiceGroup title="MCP" items={mcps} all={items} busyId={busyId} ports={ports} setPorts={setPorts} onToggle={onToggle} onBoot={onBoot} onPort={onPort} onMeta={onMeta} />
      {data?.error ? <p className="svc-err" role="alert">{data.error}</p> : null}
      <button type="button" className="btn-primary svc-start" disabled={busyId !== null} onClick={onStartSelected}>
        Arrancar selección
      </button>
    </motion.aside>
  );
}

function ServiceGroup({
  title, items, all, busyId, ports, setPorts, onToggle, onBoot, onPort, onMeta,
}: {
  title: string;
  items: Service[];
  all: Service[];
  busyId: string | null;
  ports: Record<string, string>;
  setPorts: (value: Record<string, string> | ((prev: Record<string, string>) => Record<string, string>)) => void;
  onToggle: (id: string, running: boolean) => void;
  onBoot: (id: string, on: boolean) => void;
  onPort: (id: string, port: number) => void;
  onMeta: (id: string, name: string, description: string) => void;
}) {
  if (items.length === 0) return null;
  return (
    <section className="svc-group" aria-label={title}>
      <h3>{title}</h3>
      <ul className="svc-sheet-list">
        {items.map((service) => (
          <ServiceRow
            key={service.id}
            service={service}
            peer={all.find((item) => item.id === service.peer)?.name}
            busyId={busyId}
            port={ports[service.id] ?? String(service.host_port ?? "")}
            onPortDraft={(value) => setPorts((prev) => ({ ...prev, [service.id]: value }))}
            onToggle={onToggle}
            onBoot={onBoot}
            onPort={onPort}
            onMeta={onMeta}
          />
        ))}
      </ul>
    </section>
  );
}

function ServiceRow({
  service, peer, busyId, port, onPortDraft, onToggle, onBoot, onPort, onMeta,
}: {
  service: Service;
  peer?: string;
  busyId: string | null;
  port: string;
  onPortDraft: (value: string) => void;
  onToggle: (id: string, running: boolean) => void;
  onBoot: (id: string, on: boolean) => void;
  onPort: (id: string, port: number) => void;
  onMeta: (id: string, name: string, description: string) => void;
}) {
  const [name, setName] = useState(service.name);
  const [description, setDescription] = useState(service.description ?? "");
  const busy = busyId === service.id;
  const label = busy ? "…" : service.running ? "Parar" : "Arrancar";
  const save = () => {
    if (name.trim() && (name !== service.name || description !== (service.description ?? ""))) {
      onMeta(service.id, name.trim(), description.trim());
    }
  };
  return (
    <li>
      <span className={service.healthy ? "dot on" : "dot"} aria-hidden />
      <div className="svc-sheet-copy">
        <input aria-label={`Nombre de ${service.name}`} value={name} disabled={busyId !== null} onChange={(event) => setName(event.target.value)} onBlur={save} />
        <textarea aria-label={`Descripción de ${service.name}`} rows={2} value={description} disabled={busyId !== null} onChange={(event) => setDescription(event.target.value)} onBlur={save} />
        <span>{service.detail}{peer ? ` · junto a ${peer}` : ""}</span>
        <span>
          {service.via_gateway
            ? `interno :${service.container_port ?? "—"} · vía gateway`
            : service.host_port
              ? `host :${service.host_port} → :${service.container_port ?? "—"}`
              : "sin puerto de host"}
        </span>
      </div>
      <label className="svc-boot">
        <input type="checkbox" checked={Boolean(service.autostart)} disabled={busyId !== null} onChange={(event) => onBoot(service.id, event.target.checked)} />
        Arranque
      </label>
      {!service.via_gateway && service.host_port ? (
        <form className="svc-port inline" onSubmit={(event) => {
          event.preventDefault();
          const next = Number(port);
          if (next >= 1 && next <= 65535) onPort(service.id, next);
        }}>
          <input aria-label={`Puerto de ${service.name}`} inputMode="numeric" value={port} onChange={(event) => onPortDraft(event.target.value)} />
          <button type="submit" className="btn-ghost" disabled={busyId !== null}>Puerto</button>
        </form>
      ) : null}
      <button type="button" className="btn-ghost svc-toggle" disabled={busyId !== null} onClick={() => onToggle(service.id, service.running)}>
        {label}
      </button>
    </li>
  );
}


