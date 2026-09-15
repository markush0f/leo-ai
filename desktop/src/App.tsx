/**
 * Desktop shell coordinating chat history, catalog edits, theme, and voice status.
 * Model history is separate from display bubbles so UI errors are not sent back
 * as assistant replies. Service calls go through `api.ts`.
 */
import { useCallback, useEffect, useRef, useState } from "react";
import {
  applyOp,
  inTauri,
  loadSnapshot,
  sendChat,
  voiceListen,
  voiceShutdown,
  voiceSpeak,
  voiceStatus,
  voiceStop,
} from "./api";
import { Catalog } from "./Catalog";
import {
  IconClose,
  IconMenu,
  IconMic,
  IconMoon,
  IconPlus,
  IconPower,
  IconSend,
  IconSliders,
  IconSpeak,
  IconStop,
  IconSun,
} from "./icons";
import { applyTheme, readTheme, type Theme } from "./theme";
import type { Bubble, ChatTurn, Op, Snapshot, Voice } from "./types";

const PHASE: Record<string, string> = {
  idle: "quieto",
  listening: "escuchando",
  recording: "grabando",
  transcribing: "transcribiendo",
  thinking: "pensando",
  speaking: "hablando",
  apagado: "apagado",
};

function uid() {
  return crypto.randomUUID();
}

export default function App() {
  const [snap, setSnap] = useState<Snapshot | null>(null);
  const [boot, setBoot] = useState<string | null>(null);
  const [bubbles, setBubbles] = useState<Bubble[]>([]);
  const [history, setHistory] = useState<ChatTurn[]>([]);
  const [input, setInput] = useState("");
  const [busy, setBusy] = useState(false);
  const [catalog, setCatalog] = useState(false);
  const [rail, setRail] = useState(false);
  const [theme, setTheme] = useState<Theme>("dark");
  const [thinking, setThinking] = useState(false);
  const [useTools, setUseTools] = useState(true);
  const [voice, setVoice] = useState<Voice>({
    running: false,
    ok: false,
    state: "apagado",
    message: null,
  });
  const listRef = useRef<HTMLDivElement>(null);
  const boxRef = useRef<HTMLTextAreaElement>(null);

  const refresh = useCallback(async () => {
    try {
      setSnap(await loadSnapshot());
      setBoot(null);
    } catch (e) {
      setBoot(e instanceof Error ? e.message : String(e));
    }
  }, []);

  useEffect(() => {
    const q = new URLSearchParams(window.location.search).get("theme");
    const t = q === "light" || q === "dark" ? q : readTheme();
    setTheme(t);
    applyTheme(t);
    const think = localStorage.getItem("leo-thinking");
    if (think === "1" || think === "0") setThinking(think === "1");
    const tools = localStorage.getItem("leo-tools");
    if (tools === "1" || tools === "0") setUseTools(tools === "1");
  }, []);

  useEffect(() => {
    void refresh();
    const q = new URLSearchParams(window.location.search);
    if (q.has("catalog")) setCatalog(true);
    if (q.has("demo")) {
      setBubbles([
        { id: "d1", kind: "user", text: "¿qué tiempo hace en Madrid?" },
        {
          id: "d2",
          kind: "leo",
          text: "En Madrid ahora hay 22 °C y cielo despejado. Esta tarde baja a 17 °C.",
        },
      ]);
      setHistory([
        { role: "user", content: "¿qué tiempo hace en Madrid?" },
        {
          role: "assistant",
          content: "En Madrid ahora hay 22 °C y cielo despejado. Esta tarde baja a 17 °C.",
        },
      ]);
    }
  }, [refresh]);

  useEffect(() => {
    const tick = () => {
      void voiceStatus().then(setVoice);
    };
    tick();
    const id = setInterval(tick, 2000);
    return () => clearInterval(id);
  }, []);

  useEffect(() => {
    const el = listRef.current;
    if (el) el.scrollTop = el.scrollHeight;
  }, [bubbles, busy]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        setCatalog(false);
        setRail(false);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  const onOp = async (op: Op) => {
    setSnap(await applyOp(op));
  };

  const resizeBox = () => {
    const el = boxRef.current;
    if (!el) return;
    el.style.height = "auto";
    el.style.height = `${Math.min(el.scrollHeight, 160)}px`;
  };

  const send = async () => {
    const text = input.trim();
    if (!text || busy || !snap) return;
    setInput("");
    if (boxRef.current) boxRef.current.style.height = "auto";
    const turn: ChatTurn = { role: "user", content: text };
    const next = [...history, turn];
    setHistory(next);
    setBubbles((b) => [...b, { id: uid(), kind: "user", text }]);
    setBusy(true);
    try {
      const reply = await sendChat(next, { thinking, tools: useTools });
      setHistory((h) => [...h, { role: "assistant", content: reply }]);
      setBubbles((b) => [...b, { id: uid(), kind: "leo", text: reply }]);
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      setBubbles((b) => [...b, { id: uid(), kind: "error", text: msg }]);
    } finally {
      setBusy(false);
      boxRef.current?.focus();
    }
  };

  const clear = () => {
    setBubbles([]);
    setHistory([]);
    setRail(false);
    boxRef.current?.focus();
  };

  const toggleTheme = () => {
    const next = theme === "dark" ? "light" : "dark";
    setTheme(next);
    applyTheme(next);
  };

  const lastLeo = [...bubbles].reverse().find((b) => b.kind === "leo")?.text ?? "";
  const model = snap?.models.find((m) => m.id === snap.active_model_id);
  const provider = snap?.providers.find((p) => p.id === model?.provider_id);
  const phase = PHASE[voice.state] ?? voice.state;
  const listening = voice.running && (voice.state === "listening" || voice.state === "recording");
  const chatting = bubbles.length > 0 || busy;
  const canSend = Boolean(snap) && !busy && input.trim().length > 0;

  const composer = (
    <form
      className="composer"
      onSubmit={(e) => {
        e.preventDefault();
        void send();
      }}
    >
      <textarea
        ref={boxRef}
        value={input}
        rows={1}
        disabled={!snap && !boot}
        placeholder={snap ? "Pregúntale a Leo" : "sin catálogo"}
        aria-label="mensaje"
        onChange={(e) => {
          setInput(e.target.value);
          resizeBox();
        }}
        onKeyDown={(e) => {
          if (e.key === "Enter" && !e.shiftKey) {
            e.preventDefault();
            void send();
          }
        }}
      />
      <div className="composer-bar">
        <select
          className="model-select"
          aria-label="modelo"
          disabled={!snap}
          value={snap?.active_model_id ?? ""}
          onChange={(e) => {
            const id = e.target.value;
            if (id) void onOp({ op: "activate_model", id });
          }}
        >
          {snap?.models.map((m) => {
            const p = snap.providers.find((x) => x.id === m.provider_id);
            return (
              <option key={m.id} value={m.id}>
                {m.name} · {p?.name ?? ""}
              </option>
            );
          })}
        </select>
        <button
          type="button"
          className={`mode-chip${thinking ? " on" : ""}`}
          aria-pressed={thinking}
          title="Grok no puede apagar el razonamiento; apagado = esfuerzo bajo, encendido = alto"
          onClick={() => {
            const next = !thinking;
            setThinking(next);
            localStorage.setItem("leo-thinking", next ? "1" : "0");
          }}
        >
          Pensar
        </button>
        <button
          type="button"
          className={`mode-chip${useTools ? " on" : ""}`}
          aria-pressed={useTools}
          title="Herramientas (archivos, shell, clima…)"
          onClick={() => {
            const next = !useTools;
            setUseTools(next);
            localStorage.setItem("leo-tools", next ? "1" : "0");
          }}
        >
          Tools
        </button>
        <span className="composer-grow" />
        <button
          type="button"
          className={`btn-icon ${listening ? "live" : ""}`}
          title="escuchar"
          aria-label="escuchar"
          onClick={() => void voiceListen().then(setVoice)}
        >
          <IconMic />
        </button>
        <button
          type="submit"
          className="btn-send"
          disabled={!canSend}
          aria-label="enviar"
        >
          <IconSend />
        </button>
      </div>
    </form>
  );

  return (
    <div className={`app${rail ? " rail-open" : ""}${catalog ? " sheet-open" : ""}`}>
      {rail && (
        <button
          type="button"
          className="scrim"
          aria-label="cerrar menú"
          onClick={() => setRail(false)}
        />
      )}

      <aside className="rail" aria-label="navegación">
        <div className="rail-top">
          <p className="brand">Leo</p>
          <button type="button" className="btn-ghost rail-close" onClick={() => setRail(false)}>
            <IconClose />
          </button>
        </div>

        <button type="button" className="btn-primary new-chat" onClick={clear}>
          <IconPlus />
          Nuevo chat
        </button>

        <nav className="rail-nav">
          <button
            type="button"
            className={`nav-item${catalog ? " on" : ""}`}
            onClick={() => setCatalog(true)}
          >
            <IconSliders />
            Catálogo
          </button>
        </nav>

        <section className="rail-voice" aria-label="voz">
          <header>
            <span>Voz</span>
            <em className={voice.running ? "live" : ""} title={voice.message ?? undefined}>
              {phase}
            </em>
          </header>
          <div className="voice-grid">
            <button
              type="button"
              className={`btn-listen${listening ? " on" : ""}`}
              onClick={() => void voiceListen().then(setVoice)}
            >
              <IconMic />
              Escuchar
            </button>
            <button
              type="button"
              className="btn-secondary"
              onClick={() => void voiceStop().then(setVoice)}
            >
              <IconStop />
              Parar
            </button>
            <button
              type="button"
              className="btn-secondary"
              disabled={!lastLeo}
              onClick={() => void voiceSpeak(lastLeo).then(setVoice)}
            >
              <IconSpeak />
              Decir
            </button>
            <button
              type="button"
              className="btn-danger"
              onClick={() => void voiceShutdown().then(setVoice)}
            >
              <IconPower />
              Apagar
            </button>
          </div>
        </section>

        <div className="rail-foot">
          {!inTauri && <p className="preview">vista previa</p>}
          <button type="button" className="btn-ghost theme-btn" onClick={toggleTheme}>
            {theme === "dark" ? <IconSun /> : <IconMoon />}
            {theme === "dark" ? "Modo claro" : "Modo oscuro"}
          </button>
        </div>
      </aside>

      <div className="stage">
        <header className="topbar">
          <button
            type="button"
            className="btn-ghost menu-btn"
            aria-label="menú"
            onClick={() => setRail(true)}
          >
            <IconMenu />
          </button>
          <p className="top-model">
            {model ? `${model.name}` : "Leo"}
            {provider ? <span> · {provider.name}</span> : null}
          </p>
          <button
            type="button"
            className="btn-ghost theme-btn compact"
            onClick={toggleTheme}
            aria-label={theme === "dark" ? "modo claro" : "modo oscuro"}
          >
            {theme === "dark" ? <IconSun /> : <IconMoon />}
          </button>
        </header>

        <div className={chatting ? "main chatting" : "main welcome"}>
          {!chatting && (
            <div className="hero">
              <h1>Hola</h1>
              <p>
                {boot
                  ? boot
                  : `Leo está listo${model ? ` · ${model.name}` : ""}${
                      snap?.tools.length ? ` · ${snap.tools.length} tools` : ""
                    }`}
              </p>
            </div>
          )}

          {chatting && (
            <div className="log" ref={listRef}>
              {boot && <p className="bubble error">{boot}</p>}
              {bubbles.map((b) => (
                <article key={b.id} className={`turn ${b.kind}`}>
                  {b.kind !== "user" && (
                    <span className="who">{b.kind === "error" ? "error" : "Leo"}</span>
                  )}
                  <div className={`bubble ${b.kind}`}>
                    <p>{b.text}</p>
                  </div>
                </article>
              ))}
              {busy && (
                <article className="turn leo" aria-live="polite">
                  <span className="who">Leo</span>
                  <div className="bubble leo load">
                    <span className="dots" />
                    {thinking ? "razonando" : "pensando"}
                  </div>
                </article>
              )}
            </div>
          )}

          <div className="dock">{composer}</div>
        </div>
      </div>

      {catalog && snap && (
        <>
          <button
            type="button"
            className="scrim settings"
            aria-label="cerrar catálogo"
            onClick={() => setCatalog(false)}
          />
          <Catalog snap={snap} onOp={onOp} onClose={() => setCatalog(false)} />
        </>
      )}
    </div>
  );
}
