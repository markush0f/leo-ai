import { useEffect, useRef, useState, type PointerEvent, type KeyboardEvent } from "react";

const MIN = 224;
const MAX = 360;
const clamp = (value: number) => Math.max(MIN, Math.min(MAX, value));
function read(key: string) { try { return localStorage.getItem(key); } catch { return null; } }
function write(key: string, value: string) { try { localStorage.setItem(key, value); } catch { /* Storage may be unavailable in embedded views. */ } }

export function useSidebar() {
  const [collapsed, setCollapsed] = useState(() => read("ira.rail.collapsed") === "true");
  const [width, setWidth] = useState(() => {
    const value = Number(read("ira.rail.width"));
    return value > 0 && Number.isFinite(value) ? clamp(value) : 272;
  });
  const [dragging, setDragging] = useState(false);
  const drag = useRef<{ x: number; width: number } | null>(null);
  useEffect(() => write("ira.rail.collapsed", String(collapsed)), [collapsed]);
  useEffect(() => { if (!dragging) write("ira.rail.width", String(width)); }, [width, dragging]);
  return {
    collapsed, setCollapsed, width, dragging,
    resizeProps: {
      role: "separator" as const, tabIndex: 0,
      "aria-label": "Ancho de la barra lateral", "aria-orientation": "vertical" as const,
      "aria-valuemin": MIN, "aria-valuemax": MAX, "aria-valuenow": width,
      onPointerDown: (event: PointerEvent<HTMLDivElement>) => {
        if (event.button !== 0) return;
        event.preventDefault();
        event.currentTarget.setPointerCapture(event.pointerId);
        drag.current = { x: event.clientX, width }; setDragging(true);
      },
      onPointerMove: (event: PointerEvent<HTMLDivElement>) => {
        if (drag.current) setWidth(clamp(drag.current.width + event.clientX - drag.current.x));
      },
      onPointerUp: (event: PointerEvent<HTMLDivElement>) => {
        drag.current = null; setDragging(false);
        if (event.currentTarget.hasPointerCapture(event.pointerId)) event.currentTarget.releasePointerCapture(event.pointerId);
      },
      onLostPointerCapture: () => { drag.current = null; setDragging(false); },
      onKeyDown: (event: KeyboardEvent<HTMLDivElement>) => {
        if (!["ArrowLeft", "ArrowRight", "Home", "End"].includes(event.key)) return;
        event.preventDefault();
        setWidth((value) => event.key === "Home" ? MIN : event.key === "End" ? MAX : clamp(value + (event.key === "ArrowRight" ? 16 : -16)));
      },
      onDoubleClick: () => setWidth(272),
    },
  };
}
