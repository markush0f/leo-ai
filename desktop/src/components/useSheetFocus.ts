import { useEffect, useRef } from "react";

/** Keeps keyboard navigation inside a modal sheet, then restores its opener. */
export function useSheetFocus(active = true, returnSelector = '[data-sheet-trigger="true"]') {
  const ref = useRef<HTMLElement>(null);
  useEffect(() => {
    if (!active) return;
    const previous = document.querySelector<HTMLElement>(returnSelector) ?? document.activeElement as HTMLElement | null;
    const sheet = ref.current;
    if (!sheet) return;
    const selector = 'button:not(:disabled), input:not(:disabled), select:not(:disabled), textarea:not(:disabled), summary, a[href], [tabindex="0"]';
    const visible = () => [...sheet.querySelectorAll<HTMLElement>(selector)].filter((el) => el.getClientRects().length > 0);
    (visible()[0] ?? sheet).focus();
    const trap = (event: KeyboardEvent) => {
      if (event.key !== "Tab") return;
      const elements = visible();
      const first = elements[0];
      const last = elements[elements.length - 1];
      if (!first) { event.preventDefault(); sheet.focus(); return; }
      if (event.shiftKey && (document.activeElement === first || document.activeElement === sheet)) {
        event.preventDefault(); last?.focus();
      } else if (!event.shiftKey && document.activeElement === last) {
        event.preventDefault(); first.focus();
      }
    };
    sheet.addEventListener("keydown", trap);
    return () => {
      sheet.removeEventListener("keydown", trap);
      const candidates = [previous, ...document.querySelectorAll<HTMLElement>('[data-mobile-menu="true"], [data-desktop-menu="true"]')];
      candidates.find((element) => element && element.getClientRects().length > 0 && getComputedStyle(element).visibility !== "hidden" && !element.closest("[inert]"))?.focus();
    };
  }, [active, returnSelector]);
  return ref;
}
