/**
 * Desktop shell coordinating chat history, catalog edits, host services, and theme.
 * Model history is separate from display bubbles so UI errors are not sent back
 * as assistant replies. Service calls go through `api.ts` (Tauri or ira-server).
 */
import { useCallback, useEffect, useRef, useState } from "react";
import { AnimatePresence, motion, MotionConfig } from "motion/react";
import {
  applyOp,
  beginCodexLogin,
  finishCodexLogin,
  listChats,
  loadServices,
  loadSnapshot,
  newChat,
  openChat,
  streamChat,
  startServices,
} from "./api";
import { Catalog } from "./Catalog";
import { Databases } from "./Databases";
import { Markdown } from "./Markdown";
import { ServiceBoard } from "./Services";
import {
  IconClose,
  IconDatabase,
  IconMenu,
  IconMic,
  IconMoon,
  IconPlus,
  IconSend,
  IconSliders,
  IconSun,
} from "./icons";
import { VoiceStage, type VoicePhase } from "./VoiceStage";
import { startVoice, type VoiceSession } from "./voice";
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

function mergeReply(prev: string, next: string): string {
  if (!prev) return next;
  if (next.startsWith(prev)) return next;
  if (prev.includes(next)) return prev;
  return `${prev} ${next}`.replace(/\s+/g, " ").trim();
}

function turnsToBubbles(turns: Turn[]): Bubble[] {
  const out: Bubble[] = [];
  for (const t of turns) {
    if (t.role === "user") out.push({ id: t.id, kind: "user", text: t.content });
    else if (t.role === "assistant") out.push({ id: t.id, kind: "ira", text: t.content });
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
  const [receiving, setReceiving] = useState(false);
  const [catalog, setCatalog] = useState(false);
  const [databases, setDatabases] = useState(false);
  const [rail, setRail] = useState(false);
  const [railCollapsed, setRailCollapsed] = useState(false);
  const [theme, setTheme] = useState<Theme>("dark");
  const [services, setServices] = useState<Services | null>(null);
  const [starting, setStarting] = useState(false);
  const [listening, setListening] = useState(false);
  const [voiceOpen, setVoiceOpen] = useState(false);
  const [voicePhase, setVoicePhase] = useState<VoicePhase>("connect");
  const [voiceUser, setVoiceUser] = useState("");
  const [voiceIra, setVoiceIra] = useState("");
  const [voiceError, setVoiceError] = useState<string | null>(null);
  const listRef = useRef<HTMLDivElement>(null);
  const boxRef = useRef<HTMLTextAreaElement>(null);
  const voiceRef = useRef<VoiceSession | null>(null);
  const voiceBusy = useRef(false);
  const voiceSpeaking = useRef(false);
  const voiceLevel = useRef(0);
  const iraVoiceBubble = useRef<string | null>(null);
  const voiceHangup = useRef(false);
  const chatRunRef = useRef(0);

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
            kind: "ira",
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

  const stopVoice = useCallback(() => {
    const session = voiceRef.current;
    voiceRef.current = null;
    voiceHangup.current = true;
    session?.stop();
    voiceHangup.current = false;
    voiceBusy.current = false;
    voiceSpeaking.current = false;
    voiceLevel.current = 0;
    iraVoiceBubble.current = null;
    setListening(false);
    setVoiceOpen(false);
    setVoicePhase("connect");
    setVoiceUser("");
    setVoiceIra("");
    setVoiceError(null);
  }, []);

  useEffect(() => () => stopVoice(), [stopVoice]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        setCatalog(false);
        setDatabases(false);
        setRail(false);
        stopVoice();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [stopVoice]);

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
    if (!text || busy || listening || !snap || !conversationId) return;
    setInput("");
    if (boxRef.current) boxRef.current.style.height = "auto";
    setBubbles((b) => [...b, { id: uid(), kind: "user", text }]);
    setBusy(true);
    setReceiving(false);
    const run = ++chatRunRef.current;
    const replyId = uid();
    let replyVisible = false;
    try {
      await streamChat(conversationId, text, (event) => {
        if (chatRunRef.current !== run) return;
        if (event.type === "delta") {
          if (!event.text) return;
          setReceiving(true);
          if (!replyVisible) {
            replyVisible = true;
            setBubbles((current) => [
              ...current,
              { id: replyId, kind: "ira", text: event.text },
            ]);
          } else {
            setBubbles((current) => current.map((bubble) =>
              bubble.id === replyId ? { ...bubble, text: bubble.text + event.text } : bubble,
            ));
          }
        } else if (event.type === "reset") {
          replyVisible = false;
          setReceiving(false);
          setBubbles((current) => current.filter((bubble) => bubble.id !== replyId));
        }
      });
      const nextChats = await listChats();
      if (chatRunRef.current === run) setChats(nextChats);
    } catch (e) {
      if (chatRunRef.current !== run) return;
      const msg = e instanceof Error ? e.message : String(e);
      if (replyVisible) {
        setBubbles((current) => current.filter((bubble) => bubble.id !== replyId));
      }
      setBubbles((b) => [...b, { id: uid(), kind: "error", text: msg }]);
    } finally {
      if (chatRunRef.current === run) {
        setBusy(false);
        setReceiving(false);
        boxRef.current?.focus();
      }
    }
  };

  const talk = async () => {
    if (listening || voiceOpen) {
      stopVoice();
      return;
    }
    if (busy || !snap || !conversationId) return;
    setVoiceOpen(true);
    setVoicePhase("connect");
    setVoiceUser("");
    setVoiceIra("");
    setVoiceError(null);
    iraVoiceBubble.current = null;
    try {
      setListening(true);
      voiceRef.current = await startVoice(conversationId, {
        onTranscript: (text, final) => {
          setVoiceUser(text);
          if (!final) return false;
          if (voiceBusy.current) return false;
          voiceBusy.current = true;
          iraVoiceBubble.current = null;
          setVoiceIra("");
          setVoicePhase("wait");
          setBubbles((b) => [...b, { id: uid(), kind: "user", text }]);
          setBusy(true);
          return true;
        },
        onReply: (text) => {
          setVoiceIra((prev) => mergeReply(prev, text));
          setBubbles((b) => {
            const id = iraVoiceBubble.current;
            if (id) {
              return b.map((bubble) =>
                bubble.id === id ? { ...bubble, text: mergeReply(bubble.text, text) } : bubble,
              );
            }
            const created = uid();
            iraVoiceBubble.current = created;
            return [...b, { id: created, kind: "ira", text }];
          });
          setBusy(false);
          if (!voiceSpeaking.current) voiceBusy.current = false;
          void listChats().then(setChats);
        },
        onLevel: (rms) => {
          voiceLevel.current = rms;
        },
        onSpeaking: (speaking) => {
          voiceSpeaking.current = speaking;
          if (speaking) {
            setVoicePhase("speak");
            return;
          }
          voiceBusy.current = false;
          setVoicePhase((phase) => (phase === "error" ? phase : "listen"));
        },
        onError: (message) => {
          setVoiceError(message);
          setVoicePhase("error");
          setBubbles((b) => [...b, { id: uid(), kind: "error", text: message }]);
          setBusy(false);
          voiceBusy.current = false;
        },
        onClose: () => {
          voiceRef.current = null;
          voiceBusy.current = false;
          voiceSpeaking.current = false;
          setListening(false);
          setBusy(false);
          if (!voiceHangup.current) {
            setVoicePhase("error");
            setVoiceError((err) => err ?? "conexión perdida");
          }
        },
      });
      setVoicePhase((phase) => (phase === "connect" ? "listen" : phase));
    } catch (e) {
      setListening(false);
      const msg = e instanceof Error ? e.message : String(e);
      setVoiceError(msg);
      setVoicePhase("error");
      setBubbles((b) => [...b, { id: uid(), kind: "error", text: msg }]);
    }
  };

  const clear = async () => {
    const run = ++chatRunRef.current;
    setBusy(false);
    setReceiving(false);
    stopVoice();
    try {
      const created = await newChat();
      const nextChats = await listChats();
      if (chatRunRef.current !== run) return;
      setChats(nextChats);
      setConversationId(created.id);
      setBubbles([]);
    } catch (e) {
      setBoot(e instanceof Error ? e.message : String(e));
    }
    setRail(false);
    boxRef.current?.focus();
  };

  const open = async (id: string) => {
    const run = ++chatRunRef.current;
    setBusy(false);
    setReceiving(false);
    stopVoice();
    const turns = await openChat(id);
    if (chatRunRef.current !== run) return;
    setConversationId(id);
    setBubbles(turnsToBubbles(turns));
    setRail(false);
  };

  const toggleTheme = () => {
    const next = theme === "dark" ? "light" : "dark";
    setTheme(next);
    applyTheme(next);
  };

  const openRail = () => {
    if (window.matchMedia("(max-width: 860px)").matches) setRail(true);
    else setRailCollapsed(false);
  };

  const closeRail = () => {
    if (window.matchMedia("(max-width: 860px)").matches) setRail(false);
    else setRailCollapsed(true);
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
  const chatting = bubbles.length > 0 || busy || listening;
  const canSend = Boolean(snap) && !busy && !listening && input.trim().length > 0;
  const canTalk = Boolean(snap) && Boolean(conversationId) && (listening || !busy);

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
        disabled={(!snap && !boot) || listening}
        placeholder={listening ? "Te escucho…" : snap ? "Pregúntale a Ira" : "sin catálogo"}
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
          type="button"
          className={`btn-mic${listening ? " on" : ""}`}
          disabled={!canTalk}
          aria-pressed={listening}
          aria-label={listening ? "dejar de hablar" : "hablar"}
          title={listening ? "Dejar de hablar" : "Hablar con Ira"}
          onClick={() => void talk()}
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
    <MotionConfig reducedMotion="user" transition={{ duration: 0.16, ease: [0.16, 1, 0.3, 1] }}>
    <div className={`app${rail ? " rail-open" : ""}${railCollapsed ? " rail-collapsed" : ""}${catalog || databases || voiceOpen ? " sheet-open" : ""}`}>
      <AnimatePresence>
      {rail && (
        <motion.button
          type="button"
          className="scrim"
          aria-label="cerrar menú"
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          exit={{ opacity: 0 }}
          onClick={() => setRail(false)}
        />
      )}
      </AnimatePresence>

      <aside className="rail" aria-label="navegación">
        <div className="rail-top">
          <p className="brand">
            <img src="/ira-logo.png" alt="" />
            Ira
          </p>
          <button
            type="button"
            className="btn-ghost rail-close"
            aria-label="ocultar barra lateral"
            title="Ocultar barra lateral"
            onClick={closeRail}
          >
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
            onClick={() => { setCatalog(true); setDatabases(false); setRail(false); }}
          >
            <IconSliders />
            Catálogo
          </button>
          <button
            type="button"
            className={`nav-item${databases ? " on" : ""}`}
            onClick={() => { setDatabases(true); setCatalog(false); setRail(false); }}
          >
            <IconDatabase />
            Bases de datos
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
            aria-label="mostrar barra lateral"
            title="Mostrar barra lateral"
            onClick={openRail}
          >
            <IconMenu />
          </button>
          <p className="top-model">
            {model ? `${model.name}` : "Ira"}
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
          {!chatting ? (
            <div className="hero">
              <img className="hero-logo" src="/ira-logo.png" alt="" />
              <h1>Hola</h1>
              <p>
                {boot
                  ? boot
                  : `Ira está listo${model ? ` · ${model.name}` : ""}${
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
          ) : (
            <div className="log" ref={listRef}>
              {boot && <p className="bubble error">{boot}</p>}
              <AnimatePresence initial={false}>
              {bubbles.map((b) => (
                <motion.article
                  key={b.id}
                  className={`turn ${b.kind}`}
                  initial={{ opacity: 0 }}
                  animate={{ opacity: 1 }}
                  transition={{ duration: 0.14 }}
                >
                  {b.kind !== "user" && (
                    <span className="who">{b.kind === "error" ? "error" : "Ira"}</span>
                  )}
                  <div className={`bubble ${b.kind}`}>
                    {b.kind === "ira" ? <Markdown text={b.text} /> : <p>{b.text}</p>}
                  </div>
                </motion.article>
              ))}
              </AnimatePresence>
              <AnimatePresence>
              {busy && !receiving && (
                <motion.article
                  className="turn ira"
                  aria-live="polite"
                  initial={{ opacity: 0 }}
                  animate={{ opacity: 1 }}
                  exit={{ opacity: 0 }}
                >
                  <span className="who">Ira</span>
                  <div className="bubble ira load">
                    <span className="dots" />
                    {thinking ? "razonando" : "pensando"}
                  </div>
                </motion.article>
              )}
              </AnimatePresence>
            </div>
          )}

          <div className="dock">{composer}</div>
        </div>
      </div>

      <AnimatePresence>
      {catalog && snap && (
          <motion.button
            type="button"
            className="scrim settings"
            aria-label="cerrar catálogo"
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            onClick={() => setCatalog(false)}
          />
      )}
      </AnimatePresence>
      <AnimatePresence>
      {catalog && snap && (
          <Catalog
            snap={snap}
            onOp={onOp}
            onCodexLogin={onCodexLogin}
            onClose={() => setCatalog(false)}
          />
      )}
      </AnimatePresence>
      <AnimatePresence>
      {databases && (
          <motion.button type="button" className="scrim settings" aria-label="cerrar bases de datos" initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }} onClick={() => setDatabases(false)} />
      )}
      </AnimatePresence>
      <AnimatePresence>
      {databases && (
          <Databases onClose={() => setDatabases(false)} />
      )}
      </AnimatePresence>
      <AnimatePresence>
      {voiceOpen && (
        <VoiceStage
          phase={voicePhase}
          userText={voiceUser}
          iraText={voiceIra}
          error={voiceError}
          levelRef={voiceLevel}
          onHangup={stopVoice}
        />
      )}
      </AnimatePresence>
    </div>
    </MotionConfig>
  );
}
