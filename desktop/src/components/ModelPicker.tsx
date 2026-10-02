import { useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import type { Snapshot } from "../types";
import { IconCheck, IconDown, IconSearch, IconClose } from "../icons";
import { Input } from "./Field";
import { Activity } from "./Activity";
import { btn, cx, modelSelect } from "../ui";

export function ModelPicker({ snap, disabled, onModel, onSettings }: {
  snap: Snapshot; disabled: boolean; onModel: (id: string) => Promise<void>; onSettings: () => void;
}) {
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState("");
  const [position, setPosition] = useState({ left: 16, top: 80, width: 380, maxHeight: 480 });
  const trigger = useRef<HTMLButtonElement>(null);
  const dialog = useRef<HTMLDialogElement>(null);
  const active = snap.models.find((m) => m.id === snap.active_model_id);
  useEffect(() => {
    if (!open) return;
    const place = () => {
      const box = trigger.current?.getBoundingClientRect();
      const width = Math.min(400, innerWidth - 32);
      if (!box) return;
      const above = box.top - 24;
      const below = innerHeight - box.bottom - 24;
      const upward = above >= Math.min(360, innerHeight / 2) || above >= below;
      const maxHeight = Math.max(120, Math.min(480, upward ? above : below));
      const height = Math.min(dialog.current?.getBoundingClientRect().height || maxHeight, maxHeight);
      setPosition({ left: Math.max(16, Math.min(box.left, innerWidth - width - 16)), top: Math.max(16, upward ? box.top - height - 8 : box.bottom + 8), width, maxHeight });
    };
    dialog.current?.showModal();
    place();
    dialog.current?.querySelector<HTMLInputElement>('input[type="search"]')?.focus();
    const observer = new ResizeObserver(place);
    if (dialog.current) observer.observe(dialog.current);
    window.addEventListener("resize", place);
    return () => { observer.disconnect(); window.removeEventListener("resize", place); trigger.current?.focus(); };
  }, [open]);
  const models = [...snap.models].sort((a, b) => {
    const ready = (id: string) => snap.providers.find((p) => p.id === id)?.key !== "falta";
    return Number(b.id === snap.active_model_id) - Number(a.id === snap.active_model_id) || Number(ready(b.provider_id)) - Number(ready(a.provider_id)) || a.display_name.localeCompare(b.display_name);
  }).filter((m) => `${m.display_name} ${m.name} ${snap.providers.find((p) => p.id === m.provider_id)?.name ?? ""}`.toLowerCase().includes(query.trim().toLowerCase()));
  const close = () => { if (!busy) { setOpen(false); setQuery(""); setError(""); } };
  return <>
    <button ref={trigger} type="button" className={cx(modelSelect, "inline-flex items-center gap-2 hover:bg-elevated !max-w-[min(15rem,40%)] phone:!max-w-none")} disabled={disabled} aria-label="Elegir modelo" aria-haspopup="dialog" aria-expanded={open} onClick={() => setOpen(true)}><span className="truncate">{active?.display_name ?? "Elegir modelo"}</span><IconDown /></button>
    {open && createPortal(<dialog ref={dialog} className="model-dialog" style={position} aria-label="Elegir modelo" onCancel={(e) => { e.preventDefault(); e.stopPropagation(); close(); }} onClick={(e) => { if (e.target === e.currentTarget) { const box = e.currentTarget.getBoundingClientRect(); if (e.clientX < box.left || e.clientX > box.right || e.clientY < box.top || e.clientY > box.bottom) close(); } }}>
      <div className="model-dialog-head"><h2>Elige un modelo</h2><button type="button" className={btn.icon} aria-label="Cerrar selector de modelos" disabled={busy !== null} onClick={close}><IconClose /></button></div>
      <Input autoFocus icon={<IconSearch />} label="Buscar modelo" type="search" placeholder="Nombre o proveedor…" value={query} disabled={busy !== null} onChange={(e) => setQuery(e.target.value)} />
      <div className="model-options" onKeyDown={(e) => {
        if (e.key !== "ArrowDown" && e.key !== "ArrowUp") return;
        const options = [...e.currentTarget.querySelectorAll<HTMLButtonElement>("button:not(:disabled)")];
        const index = options.indexOf(document.activeElement as HTMLButtonElement);
        e.preventDefault(); options[(index + (e.key === "ArrowDown" ? 1 : -1) + options.length) % options.length]?.focus();
      }}>
        {models.map((model) => {
          const provider = snap.providers.find((p) => p.id === model.provider_id);
          const missing = provider?.key === "falta";
          return <button key={model.id} type="button" className="model-option" disabled={busy !== null || missing} aria-pressed={model.id === snap.active_model_id} aria-label={`${model.id === snap.active_model_id ? "Modelo en uso:" : "Usar modelo"} ${model.display_name}`} onClick={() => {
            if (model.id === snap.active_model_id) { close(); return; }
            setBusy(model.id); setError("");
            void onModel(model.id).then(() => { setOpen(false); setQuery(""); }).catch((e) => setError(e instanceof Error ? e.message : String(e))).finally(() => setBusy(null));
          }}><span className="min-w-0 flex-1"><strong>{model.display_name || model.name}</strong><small>{provider?.name}{missing ? " · Configura credenciales" : ""}{model.context_window ? ` · ${(model.context_window / 1000).toLocaleString()}k contexto` : ""}{model.reasoning ? " · Razonamiento" : ""}</small></span>{busy === model.id ? <Activity /> : model.id === snap.active_model_id ? <IconCheck /> : null}</button>;
        })}
        {models.length === 0 && <p className="py-4 text-sm text-muted">{query ? "No hay modelos con ese nombre. Prueba otra búsqueda." : "Añade un proveedor para cargar su catálogo de modelos."}</p>}
      </div>
      {error && <p className="resource-result text-danger" role="alert">{error}</p>}
      <button type="button" className={cx(btn.ghost, "mt-2 w-full")} disabled={busy !== null} onClick={() => { setOpen(false); onSettings(); }}>Configurar proveedores</button>
    </dialog>, document.body)}
  </>;
}
