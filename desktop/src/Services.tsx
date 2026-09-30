/**
 * Host service status. Each Compose service can be started or stopped.
 */
import { useState } from "react";
import { motion } from "motion/react";
import { IconClose } from "./icons";
import { useSheetFocus } from "./components/useSheetFocus";
import type { Service, Services } from "./types";
import { btn, cx, dot, dotOn, sheet, sheetHead } from "./ui";
import { McpConnections } from "./McpConnections";

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
    <section className="flex flex-col gap-[0.45rem] rounded-[10px] border border-line bg-elevated px-[0.7rem] pt-[0.6rem] pb-[0.75rem]" aria-label="Servicios">
      <p className="m-0 text-[0.9rem] font-semibold">Servicios</p>
      <ul className="m-0 flex list-none flex-col gap-[0.35rem] p-0">
        {items.map((s) => {
          const busy = busyId === s.id;
          const label = busy ? "…" : s.running ? "Parar" : "Arrancar";
          return (
            <li key={s.id} className="flex items-center gap-[0.45rem] text-[0.85rem] font-medium" title={`${s.name}: ${s.detail}`}>
              <span className={cx(dot, s.healthy && dotOn)} aria-hidden />
              <span>{s.name}</span>
              <span className={cx("max-w-32 truncate text-xs font-medium text-muted", compact && "hidden")}>{s.detail}</span>
              <button
                type="button"
                className={cx(btn.ghost, "ml-auto min-h-7 px-2 text-xs")}
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
      {data?.error ? <p className="m-0 text-[0.8rem] text-danger">{data.error}</p> : null}
    </section>
  );
}

function isMcp(service: Service) {
  return service.kind === "mcp" || service.id.endsWith("-mcp");
}

type SheetProps = {
  data: Services | null;
  tab: "service" | "mcp";
  onTab: (tab: "service" | "mcp") => void;
  busyId: string | null;
  onClose: () => void;
  onToggle: (id: string, running: boolean) => void;
  onBoot: (id: string, on: boolean) => void;
  onPort: (id: string, port: number) => void;
  onMeta: (id: string, name: string, description: string) => void;
  onStartSelected: () => void;
};

export function ServicesSheet({ data, tab, onTab, busyId, onClose, onToggle, onBoot, onPort, onMeta, onStartSelected }: SheetProps) {
  const sheetRef = useSheetFocus();
  const items = data?.services.length ? data.services : FALLBACK;
  const services = items.filter((item) => !isMcp(item));
  const mcps = items.filter(isMcp);
  const visible = tab === "mcp" ? mcps : services;
  const [gateway, setGateway] = useState(String(data?.gateway_port ?? 8790));
  const [ports, setPorts] = useState<Record<string, string>>({});
  return (
    <motion.aside
      ref={sheetRef}
      role="dialog"
      aria-modal="true"
      tabIndex={-1}
      className={sheet}
      aria-label={tab === "mcp" ? "MCP" : "Servicios"}
      initial={{ x: "100%" }}
      animate={{ x: 0 }}
      exit={{ x: "100%" }}
      transition={{ type: "spring", stiffness: 360, damping: 38 }}
    >
      <header className={sheetHead}>
        <div>
          <h2>{tab === "mcp" ? "MCP" : "Servicios"}</h2>
          <p>{tab === "mcp" ? "Herramientas que Ira puede llamar. El par local arranca con su servicio." : "Procesos locales. El MCP de cada uno está en su propia sección."}</p>
        </div>
        <button type="button" className={btn.icon} aria-label="Cerrar servicios" title="Cerrar" onClick={onClose}>
          <IconClose />
        </button>
      </header>
      <div className="mt-[0.8rem] flex gap-[0.4rem]" role="tablist" aria-label="Tipo">
        <button type="button" role="tab" aria-selected={tab === "service"} className={cx("min-h-8 rounded-full border border-line bg-transparent px-3 text-muted", tab === "service" && "bg-elevated text-ink")} onClick={() => onTab("service")}>Servicios</button>
        <button type="button" role="tab" aria-selected={tab === "mcp"} className={cx("min-h-8 rounded-full border border-line bg-transparent px-3 text-muted", tab === "mcp" && "bg-elevated text-ink")} onClick={() => onTab("mcp")}>MCP</button>
      </div>
      {tab === "service" && <form className="mt-[0.4rem] flex items-center gap-2 [&_label]:text-[0.85rem] [&_label]:font-[550] [&_input]:h-8 [&_input]:w-[5.5rem] [&_input]:rounded-lg [&_input]:border [&_input]:border-line [&_input]:bg-elevated [&_input]:px-[0.45rem]" onSubmit={(event) => {
        event.preventDefault();
        const port = Number(gateway);
        if (port >= 1 && port <= 65535) onPort("ira-gateway", port);
      }}>
        <label htmlFor="gateway-port">Puerto expuesto del gateway</label>
        <input id="gateway-port" inputMode="numeric" value={gateway} onChange={(event) => setGateway(event.target.value)} />
        <button type="submit" className={btn.ghost} disabled={busyId !== null}>Aplicar</button>
      </form>}
      {tab === "service" ? <p className="my-[0.8rem] text-[0.85rem] text-muted">Marca qué sube en cada arranque. Un par local arranca y para junto.</p> : mcps.length > 0 ? <p className="my-[0.8rem] text-[0.85rem] text-muted">Los MCP locales arrancan con su servicio.</p> : null}
      {visible.length === 0 ? tab === "service" && <p className="my-[0.8rem] text-[0.85rem] text-muted">Ningún servicio.</p> : <ServiceGroup title={tab === "mcp" ? "MCP locales" : "Servicios"} items={visible} all={items} busyId={busyId} ports={ports} setPorts={setPorts} onToggle={onToggle} onBoot={onBoot} onPort={onPort} onMeta={onMeta} />}
      {tab === "mcp" && <McpConnections />}
      {data?.error ? <p className="m-0 text-[0.8rem] text-danger" role="alert">{data.error}</p> : null}
      {(tab === "service" || mcps.length > 0) && <button type="button" className={cx(btn.primary, "mt-4 w-full disabled:opacity-100 disabled:text-muted")} disabled={busyId !== null} onClick={onStartSelected}>
        Arrancar selección
      </button>}
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
    <section aria-label={title}>
      <h3 className="mt-4 mb-[0.45rem] text-[0.82rem] text-muted">{title}</h3>
      <ul className="m-0 flex list-none flex-col gap-[0.65rem] p-0">
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
    <li className="flex flex-wrap items-center gap-[0.55rem] rounded-[10px] border border-line p-[0.7rem]">
      <span className={cx(dot, service.healthy && dotOn)} aria-hidden />
      <div className="flex min-w-40 flex-1 flex-col gap-[0.15rem] [&_input]:w-full [&_input]:rounded-lg [&_input]:border [&_input]:border-line [&_input]:bg-elevated [&_input]:px-2 [&_input]:py-[0.35rem] [&_textarea]:min-h-[2.6rem] [&_textarea]:w-full [&_textarea]:resize-y [&_textarea]:rounded-lg [&_textarea]:border [&_textarea]:border-line [&_textarea]:bg-elevated [&_textarea]:px-2 [&_textarea]:py-[0.35rem] [&_span]:text-xs [&_span]:text-muted">
        <span className="flex items-center gap-[0.4rem] text-[0.7rem]"><span className="rounded-full border border-line px-[0.4rem] py-[0.05rem] text-[0.62rem] tracking-[0.04em] uppercase">Local</span>{service.id}</span>
        <label className="flex flex-col gap-[0.15rem]">
          <span className="text-[0.68rem] text-muted">Nombre</span>
          <input aria-label={`Nombre de ${service.id}`} placeholder="Nombre" value={name} disabled={busyId !== null} onChange={(event) => setName(event.target.value)} onBlur={save} />
        </label>
        <label className="flex flex-col gap-[0.15rem]">
          <span className="text-[0.68rem] text-muted">Descripción</span>
          <textarea aria-label={`Descripción de ${service.id}`} placeholder="Descripción" rows={2} value={description} disabled={busyId !== null} onChange={(event) => setDescription(event.target.value)} onBlur={save} />
        </label>
        <span>{service.detail}{peer ? ` · junto a ${peer}` : ""}</span>
        <span>
          {service.via_gateway
            ? `interno :${service.container_port ?? "—"} · vía gateway`
            : service.host_port
              ? `host :${service.host_port} → :${service.container_port ?? "—"}`
              : "sin puerto de host"}
        </span>
      </div>
      <label className="flex items-center gap-[0.35rem] text-[0.8rem]">
        <input type="checkbox" checked={Boolean(service.autostart)} disabled={busyId !== null} onChange={(event) => onBoot(service.id, event.target.checked)} />
        Arranque
      </label>
      {!service.via_gateway && service.host_port ? (
        <form className="flex items-center gap-2 [&_input]:h-8 [&_input]:w-[5.5rem] [&_input]:rounded-lg [&_input]:border [&_input]:border-line [&_input]:bg-elevated [&_input]:px-[0.45rem]" onSubmit={(event) => {
          event.preventDefault();
          const next = Number(port);
          if (next >= 1 && next <= 65535) onPort(service.id, next);
        }}>
          <input aria-label={`Puerto de ${service.name}`} inputMode="numeric" value={port} onChange={(event) => onPortDraft(event.target.value)} />
          <button type="submit" className={btn.ghost} disabled={busyId !== null}>Puerto</button>
        </form>
      ) : null}
      <button type="button" className={cx(btn.ghost, "ml-auto min-h-7 bg-elevated px-2 text-xs")} disabled={busyId !== null} onClick={() => onToggle(service.id, service.running)}>
        {label}
      </button>
    </li>
  );
}
