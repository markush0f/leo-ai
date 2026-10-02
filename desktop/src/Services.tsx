import { useLayoutEffect, useRef, useState } from "react";
import type { Service, Services } from "./types";
import { McpConnections } from "./McpConnections";
import { Databases } from "./Databases";
import { Workspace } from "./components/Workspace";
import { Input, TextArea } from "./components/Field";
import { Status } from "./components/Status";
import { IconPower } from "./icons";
import { btn } from "./ui";

export type ConnectionsTab = "all" | "mcp" | "database" | "service";
type Props = {
  data: Services | null; tab: ConnectionsTab; onTab: (tab: ConnectionsTab) => void;
  busyId: string | null; onClose: () => void;
  onToggle: (id: string, running: boolean) => void;
  onBoot: (id: string, on: boolean) => void;
  onPort: (id: string, port: number) => void;
  onMeta: (id: string, name: string, description: string) => void;
  onStartSelected: () => void;
};

export function ServicesSheet(props: Props) {
  const { tab, onTab, onClose } = props;
  const contentRef = useRef<HTMLDivElement>(null);
  useLayoutEffect(() => { if (contentRef.current) contentRef.current.scrollTop = 0; }, [tab]);
  return <Workspace title="Conexiones" description="Herramientas, datos y servicios que amplían lo que Ira puede hacer." onClose={onClose}>
    <nav className="task-nav" aria-label="Filtrar conexiones">
      {([["all", "Todas"], ["mcp", "Servidores MCP"], ["database", "Bases de datos"], ["service", "Sistema de Ira"]] as const).map(([value, label]) => <button key={value} type="button" aria-pressed={tab === value} onClick={() => onTab(value)}>{label}</button>)}
    </nav>
    <div ref={contentRef} className="connections-content">
      <div hidden={tab !== "all" && tab !== "mcp"}><McpConnections onStart={() => onTab("mcp")} /></div>
      <div hidden={tab !== "all" && tab !== "database"}><Databases onStart={() => onTab("database")} /></div>
      <div hidden={tab !== "service"}><SystemServices {...props} /></div>
    </div>
  </Workspace>;
}

function SystemServices({ data, busyId, onToggle, onBoot, onPort, onMeta, onStartSelected }: Props) {
  const [gateway, setGateway] = useState(String(data?.gateway_port ?? 8790));
  const items = data?.services ?? [];
  const ready = items.filter((item) => item.healthy).length;
  return <section className="resource-section" aria-label="Sistema de Ira">
    <div className="resource-heading"><div><h2>Sistema de Ira</h2><p>Almacenamiento interno y procesos locales. {ready} de {items.length} disponibles.</p></div><button type="button" className={btn.primary} disabled={busyId !== null || !data} onClick={onStartSelected}>{busyId === "boot" ? "Iniciando…" : "Iniciar servicios de arranque"}</button></div>
    <p className="resource-description mb-4">Los servicios marcados para inicio automático se arrancan juntos. Las bases de datos que consultas se gestionan en «Bases de datos».</p>
    {!data && <p role="status" className="resource-result text-muted">No se pudo cargar el estado del sistema. La disponibilidad aún no está confirmada.</p>}
    <ul className="resource-list">{items.map((service) => <ServiceRow key={service.id} service={service} busyId={busyId} onToggle={onToggle} onBoot={onBoot} onPort={onPort} onMeta={onMeta} />)}</ul>
    <details className="advanced"><summary>Avanzado · gateway</summary><form className="advanced-body" onSubmit={(e) => { e.preventDefault(); onPort("ira-gateway", Number(gateway)); }}><Input label="Puerto del gateway" type="number" required min="1" max="65535" disabled={busyId !== null} value={gateway} onChange={(e) => setGateway(e.target.value)} /><button type="submit" className={btn.secondary} disabled={busyId !== null}>Aplicar puerto</button></form></details>
    {data?.error && <p className="resource-result text-danger" role="alert">{data.error}</p>}
  </section>;
}

function ServiceRow({ service, busyId, onToggle, onBoot, onPort, onMeta }: {
  service: Service; busyId: string | null;
  onToggle: Props["onToggle"]; onBoot: Props["onBoot"]; onPort: Props["onPort"]; onMeta: Props["onMeta"];
}) {
  const [name, setName] = useState(service.name);
  const [description, setDescription] = useState(service.description ?? "");
  const [port, setPort] = useState(String(service.host_port ?? ""));
  const busy = busyId === service.id;
  return <li className="resource-row">
    <div className="resource-top"><span className="resource-icon"><IconPower /></span><div className="resource-info"><strong>{service.name}</strong><p className="resource-description">{service.id === "postgres" ? "Historial, modelos y configuración de Ira" : service.description || (service.kind === "mcp" ? "Herramientas locales" : "Proceso local")}</p>
      <Status busy={busy} tone={service.healthy ? "success" : service.running ? "error" : "neutral"}>{busy ? "Aplicando cambio…" : service.healthy ? "Disponible" : service.running ? "Iniciado · no disponible" : "Detenido"}</Status>
    </div><div className="resource-actions"><button type="button" className={btn.secondary} disabled={busyId !== null} onClick={() => onToggle(service.id, service.running)}>{service.running ? "Detener" : "Iniciar"}</button></div></div>
    <details className="advanced"><summary>Configuración y diagnóstico</summary><div className="advanced-body">
      <p className="resource-description mb-4">{service.detail}{service.peer ? ` · Vinculado a ${service.peer}` : ""}</p>
      <label className="flex items-center gap-2 mb-5 text-sm"><input type="checkbox" disabled={busyId !== null} checked={Boolean(service.autostart)} onChange={(e) => onBoot(service.id, e.target.checked)} />Iniciar automáticamente con Ira</label>
      <form onSubmit={(e) => { e.preventDefault(); onMeta(service.id, name.trim(), description.trim()); }}><Input label={`Nombre de ${service.id}`} required disabled={busyId !== null} value={name} onChange={(e) => setName(e.target.value)} /><TextArea label={`Descripción de ${service.id}`} disabled={busyId !== null} value={description} rows={2} onChange={(e) => setDescription(e.target.value)} /><button type="submit" className={btn.secondary} disabled={busyId !== null || !name.trim()}>Guardar detalles</button></form>
      {service.via_gateway ? <p className="resource-description mt-4">Puerto interno {service.container_port ?? "—"} · acceso por gateway</p> : service.host_port ? <form className="mt-5" onSubmit={(e) => { e.preventDefault(); onPort(service.id, Number(port)); }}><Input label={`Puerto de ${service.name}`} type="number" required min="1" max="65535" disabled={busyId !== null} value={port} onChange={(e) => setPort(e.target.value)} /><button type="submit" className={btn.secondary} disabled={busyId !== null}>Aplicar puerto</button></form> : null}
    </div></details>
  </li>;
}
