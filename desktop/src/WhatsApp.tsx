import { useEffect, useRef, useState, type FormEvent } from "react";
import { motion } from "motion/react";
import { loadWhatsApp, pairWhatsApp, saveWhatsAppAllow, startWhatsApp } from "./api";
import { Activity } from "./components/Activity";
import { useSheetFocus } from "./components/useSheetFocus";
import { IconClose, IconDelete, IconPlus } from "./icons";
import type { WhatsAppStatus } from "./types";

type Props = { onClose: () => void };

function label(status: WhatsAppStatus | null): string {
  switch (status?.phase) {
    case "open":
      return "Vinculado";
    case "qr":
      return "Esperando el código";
    case "connecting":
      return "Conectando";
    case "closed":
      return "Reconectando";
    default:
      return "Puente parado";
  }
}

function numberOf(user: string | null): string {
  if (!user) return "Sin número";
  const digits = user.split("@")[0]?.split(":")[0]?.replace(/\D/g, "") ?? "";
  return digits ? `+${digits}` : user;
}

function digitsOf(raw: string): string {
  return raw.replace(/\D/g, "");
}

export function WhatsApp({ onClose }: Props) {
  const sheetRef = useSheetFocus();
  const [status, setStatus] = useState<WhatsAppStatus | null>(null);
  const [phones, setPhones] = useState<string[]>([]);
  const [draft, setDraft] = useState("");
  const [fieldErr, setFieldErr] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const busyRef = useRef(false);
  const [confirmPair, setConfirmPair] = useState(false);
  const [err, setErr] = useState<string | null>(null);

  const refresh = async () => {
    const next = await loadWhatsApp();
    setStatus(next);
    if (!busyRef.current) setPhones(next.allow_phones);
    return next;
  };

  useEffect(() => {
    let stop = false;
    const tick = () => {
      void refresh().catch((error: unknown) => {
        if (!stop) setErr(error instanceof Error ? error.message : String(error));
      });
    };
    tick();
    const id = window.setInterval(tick, 2000);
    return () => {
      stop = true;
      window.clearInterval(id);
    };
  }, []);

  const run = async (action: () => Promise<WhatsAppStatus>) => {
    busyRef.current = true;
    setBusy(true);
    setErr(null);
    try {
      const next = await action();
      setStatus(next);
      setPhones(next.allow_phones);
      if (next.error) setErr(next.error);
    } catch (error) {
      setErr(error instanceof Error ? error.message : String(error));
    } finally {
      busyRef.current = false;
      setBusy(false);
    }
  };

  const addPhone = async (event: FormEvent) => {
    event.preventDefault();
    const digits = digitsOf(draft);
    if (digits.length < 8 || digits.length > 15) {
      setFieldErr("Escribe el número con prefijo, por ejemplo +34600000000");
      return;
    }
    setFieldErr(null);
    if (phones.includes(digits)) {
      setDraft("");
      return;
    }
    const next = [...phones, digits];
    await run(async () => {
      const saved = await saveWhatsAppAllow(next.join(","));
      setDraft("");
      return saved;
    });
  };

  const removePhone = (phone: string) => {
    const next = phones.filter((item) => item !== phone);
    void run(() => saveWhatsAppAllow(next.join(",")));
  };

  const linked = status?.phase === "open";
  const showQr = status?.phase === "qr" && status.qr;

  return (
    <motion.aside
      ref={sheetRef}
      role="dialog"
      aria-modal="true"
      tabIndex={-1}
      className="sheet whatsapp"
      aria-label="WhatsApp"
      initial={{ x: "100%" }}
      animate={{ x: 0 }}
      exit={{ x: "100%" }}
      transition={{ type: "spring", stiffness: 360, damping: 38 }}
    >
      <header className="sheet-head">
        <div>
          <h2>WhatsApp</h2>
          <p>Enlaza el número que responde como Leo.</p>
        </div>
        <button type="button" className="btn-ghost icon-button" aria-label="Cerrar WhatsApp" title="Cerrar" onClick={onClose}>
          <IconClose />
        </button>
      </header>

      <div className="catalog-current">
        <span className={linked ? "dot on" : "dot"} aria-hidden />
        <div>
          <span>{label(status)}</span>
          <strong>{linked ? numberOf(status?.user ?? null) : "Sin vincular"}</strong>
        </div>
      </div>

      {status?.phase === "connecting" || status?.phase === "closed" ? <Activity /> : null}

      {showQr ? (
        <figure className="wa-qr">
          <img src={status.qr ?? undefined} alt="Código para vincular WhatsApp" />
          <figcaption>WhatsApp → Dispositivos vinculados → Vincular dispositivo.</figcaption>
        </figure>
      ) : null}

      {!status?.ok ? (
        <button type="button" className="btn-primary" disabled={busy} onClick={() => void run(startWhatsApp)}>
          {busy ? "Arrancando…" : "Arrancar puente"}
        </button>
      ) : null}

      <form className={`ira-field wa-add${fieldErr ? " invalid" : ""}`} onSubmit={(event) => void addPhone(event)}>
        <label htmlFor="wa-phone">Números que pueden escribirle</label>
        <div className="wa-add-row">
          <div className="ira-control">
            <input
              id="wa-phone"
              inputMode="tel"
              autoComplete="tel"
              placeholder="+34…"
              value={draft}
              disabled={!status?.ok || busy}
              aria-invalid={fieldErr ? true : undefined}
              aria-describedby="wa-phone-help"
              onChange={(event) => {
                setFieldErr(null);
                setDraft(event.target.value);
              }}
            />
          </div>
          <button type="submit" className="btn-primary wa-add-btn" disabled={!status?.ok || busy} aria-label="Añadir número" title="Añadir número">
            <IconPlus />
          </button>
        </div>
        <small id="wa-phone-help" role={fieldErr ? "alert" : undefined}>{fieldErr || "Vacío: solo Mensajes a ti mismo."}</small>
      </form>
      {phones.length === 0 ? (
        <p className="wa-empty">Todavía no hay números. Solo tú, en Mensajes a ti mismo.</p>
      ) : (
        <ul className="wa-phones" aria-label="Números añadidos">
          {phones.map((phone) => (
            <li key={phone}>
              <span>+{phone}</span>
              <button type="button" className="btn-ghost icon-button" aria-label={`Quitar +${phone}`} title="Quitar" disabled={busy} onClick={() => removePhone(phone)}>
                <IconDelete />
              </button>
            </li>
          ))}
        </ul>
      )}
      <div className="wa-actions">
        {confirmPair ? (
          <div className="confirm">
            <span>¿Desvincular este número y mostrar otro código?</span>
            <button type="button" className="btn-danger sm" disabled={busy || !status?.ok} onClick={() => void run(pairWhatsApp).then(() => setConfirmPair(false))}>
              Cambiar número
            </button>
            <button type="button" className="btn-secondary sm" onClick={() => setConfirmPair(false)}>
              Cancelar
            </button>
          </div>
        ) : (
          <button type="button" className="btn-danger" disabled={busy || !status?.ok} onClick={() => setConfirmPair(true)}>
            Cambiar número
          </button>
        )}
      </div>

      <p className="wa-note">Leo no contesta el resto del inbox. Vincular un número puede hacer que WhatsApp lo bloquee.</p>
      {err ? <p className="sheet-err" role="alert">{err}</p> : null}
    </motion.aside>
  );
}
