/**
 * Desktop shell coordinating chat history, catalog edits, host services, and theme.
 * Model history is separate from display bubbles so UI errors are not sent back
 * as assistant replies. Service calls go through `api.ts` (Tauri or leo-server).
 */
import { useCallback, useEffect, useRef, useState } from "react";
import {
  applyOp,
  beginCodexLogin,
  finishCodexLogin,
  listChats,
  loadServices,
  loadSnapshot,
  newChat,
  openChat,
  sendChat,
  startServices,
} from "./api";
import { Catalog } from "./Catalog";
import { Markdown } from "./Markdown";
import { ServiceBoard } from "./Services";
import {
  IconClose,
  IconMenu,
  IconMoon,
  IconPlus,
  IconSend,
  IconSliders,
  IconSun,
} from "./icons";
import { applyTheme, readTheme, type Theme } from "./theme";
import type {
  Bubble,
  CodexLogin,
  Conversation,
  Op,
  Services,
  Snapshot,
  Turn,
} from "./types";

function uid() {
  return crypto.randomUUID();
}

function turnsToBubbles(turns: Turn[]): Bubble[] {
  const out: Bubble[] = [];
  for (const t of turns) {
    if (t.role === "user") out.push({ id: t.id, kind: "user", text: t.content });
    else if (t.role === "assistant") out.push({ id: t.id, kind: "leo", text: t.content });
    else if (t.role === "error") out.push({ id: t.id, kind: "error", text: t.content });
  }
  return out;
}

export default function App() {
  const [snap, setSnap] = useState<Snapshot | null>(null);
  const [boot, setBoot] = useState<string | null>(null);
  const [bubbles, setBubbles] = useState<Bubble[]>([]);
  const [chats, setChats] = useState<Conversation[]>([]);
  const [conversationId, setConversationId] = useState<string | null>(null);
  const [input, setInput] = useState("");
  const [busy, setBusy] = useState(false);
  const [catalog, setCatalog] = useState(false);
  const [rail, setRail] = useState(false);
  const [theme, setTheme] = useState<Theme>("dark");
  const [services, setServices] = useState<Services | null>(null);
  const [starting, setStarting] = useState(false);
  const listRef = useRef<HTMLDivElement>(null);
  const boxRef = useRef<HTMLTextAreaElement>(null);

  const refresh = useCallback(async () => {
    try {
      setServices(await loadServices());
    } catch {
      setServices(null);
    }
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
  }, []);

  useEffect(() => {
    void (async () => {
      await refresh();
      const q = new URLSearchParams(window.location.search);
      if (q.has("catalog")) setCatalog(true);
      if (q.has("demo")) {
        setBubbles([
          { id: "d1", kind: "user", text: "¿qué tiempo hace en Madrid?" },
          {
            id: "d2",
            kind: "leo",
            text: "En Madrid ahora hay **22 °C** y cielo despejado.\n\n- Mañana: 20 °C\n- Tarde: **17 °C**\n\n`get_weather` cubre más ciudades.",
          },
        ]);
        return;
      }
      try {
        const listed = await listChats();
        if (listed.length === 0) {
          const created = await newChat();
          setChats([created]);
          setConversationId(created.id);
          setBubbles([]);
          return;
        }
        setChats(listed);
        const active = listed[0].id;
        const turns = await openChat(active);
        setConversationId(active);
        setBubbles(turnsToBubbles(turns));
      } catch (e) {
        setBoot(e instanceof Error ? e.message : String(e));
      }
    })();
  }, [refresh]);

  useEffect(() => {
    const el = listRef.current;
    if (el) el.scrollTop = el.scrollHeight;
  }, [bubbles, busy]);

  useEffect(() => {
    if (services?.ok && !starting) return;
    const id = window.setInterval(() => {
      void loadServices()
        .then(setServices)
        .catch(() => undefined);
    }, 4000);
    return () => window.clearInterval(id);
  }, [services?.ok, starting]);

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

  const onCodexLogin = async (
    providerId: string,
    onReady: (login: CodexLogin) => void,
  ) => {
    const login = await beginCodexLogin(providerId);
    onReady(login);
    setSnap(await finishCodexLogin(login.id));
  };

  const resizeBox = () => {
    const el = boxRef.current;
    if (!el) return;
    el.style.height = "auto";
    el.style.height = `${Math.min(el.scrollHeight, 160)}px`;
  };

  const send = async () => {
    const text = input.trim();
    if (!text || busy || !snap || !conversationId) return;
    setInput("");
    if (boxRef.current) boxRef.current.style.height = "auto";
    setBubbles((b) => [...b, { id: uid(), kind: "user", text }]);
    setBusy(true);
    try {
      const reply = await sendChat(conversationId, text);
      setBubbles((b) => [...b, { id: uid(), kind: "leo", text: reply }]);
      setChats(await listChats());
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      setBubbles((b) => [...b, { id: uid(), kind: "error", text: msg }]);
    } finally {
      setBusy(false);
      boxRef.current?.focus();
    }
  };

  const clear = async () => {
    try {
      const created = await newChat();
      setChats(await listChats());
      setConversationId(created.id);
      setBubbles([]);
    } catch (e) {
      setBoot(e instanceof Error ? e.message : String(e));
    }
    setRail(false);
    boxRef.current?.focus();
  };

  const open = async (id: string) => {
    const turns = await openChat(id);
    setConversationId(id);
    setBubbles(turnsToBubbles(turns));
    setRail(false);
  };

  const toggleTheme = () => {
    const next = theme === "dark" ? "light" : "dark";
    setTheme(next);
    applyTheme(next);
  };

  const bootServices = async () => {
    setStarting(true);
    try {
      const next = await startServices();
      setServices(next);
      if (next.error) setBoot(next.error);
      await refresh();
      if (!conversationId) {
        try {
          const listed = await listChats();
          if (listed.length === 0) {
            const created = await newChat();
            setChats([created]);
            setConversationId(created.id);
          } else {
            setChats(listed);
            const active = listed[0].id;
            const turns = await openChat(active);
            setConversationId(active);
            setBubbles(turnsToBubbles(turns));
          }
        } catch (e) {
          setBoot(e instanceof Error ? e.message : String(e));
        }
      }
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      setBoot(msg);
      setServices((prev) => ({
        ok: false,
        services: prev?.services ?? [],
        error: msg,
      }));
    } finally {
      setStarting(false);
    }
  };

  const thinking = snap?.thinking ?? false;
  const useTools = snap?.tools_enabled ?? true;
  const model = snap?.models.find((m) => m.id === snap.active_model_id);
  const provider = snap?.providers.find((p) => p.id === model?.provider_id);
  const providerModels = snap?.models.filter((m) => m.provider_id === provider?.id) ?? [];
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
          {providerModels.map((m) => (
            <option key={m.id} value={m.id}>
              {m.name}
            </option>
          ))}
        </select>
        <button
          type="button"
          className={`mode-chip${thinking ? " on" : ""}`}
          aria-pressed={thinking}
          title="Grok no puede apagar el razonamiento; apagado = esfuerzo bajo, encendido = alto"
          onClick={() => {
            void onOp({ op: "set_thinking", value: !thinking });
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
            void onOp({ op: "set_tools_enabled", value: !useTools });
          }}
        >
          Tools
        </button>
        <span className="composer-grow" />
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

        <button type="button" className="btn-primary new-chat" onClick={() => void clear()}>
          <IconPlus />
          Nuevo chat
        </button>

        <nav className="rail-chats" aria-label="conversaciones">
          {chats.map((c) => (
            <button
              key={c.id}
              type="button"
              className={`chat-item${c.id === conversationId ? " on" : ""}`}
              onClick={() => void open(c.id)}
            >
              {c.title?.trim() || "Nuevo chat"}
            </button>
          ))}
        </nav>

        <nav className="rail-nav">
          <button
            type="button"
            className={`nav-item${catalog ? " on" : ""}`}
            onClick={() => { setCatalog(true); setRail(false); }}
          >
            <IconSliders />
            Catálogo
          </button>
        </nav>

        <ServiceBoard
          data={services}
          starting={starting}
          compact
          onStart={() => void bootServices()}
        />

        <div className="rail-foot">
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
              {(boot || (services && !services.ok)) && (
                <ServiceBoard
                  data={services}
                  starting={starting}
                  onStart={() => void bootServices()}
                />
              )}
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
                    {b.kind === "leo" ? <Markdown text={b.text} /> : <p>{b.text}</p>}
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
          <Catalog
            snap={snap}
            onOp={onOp}
            onCodexLogin={onCodexLogin}
            onClose={() => setCatalog(false)}
          />
        </>
      )}
    </div>
  );
}
