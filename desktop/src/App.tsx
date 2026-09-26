/**
 * Desktop shell coordinating chat history, catalog edits, host services, and theme.
 * Model history is separate from display bubbles so UI errors are not sent back
 * as assistant replies. Service calls go through `api.ts` (Tauri or ira-server).
 */
import { useCallback, useEffect, useRef, useState, type CSSProperties } from "react";
import { AnimatePresence, motion, MotionConfig, useReducedMotion } from "motion/react";
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
import { WhatsApp } from "./WhatsApp";
import { Message } from "./components/Message";
import { Composer } from "./components/Composer";
import { Activity } from "./components/Activity";
import { useSidebar } from "./components/useSidebar";
import { useSheetFocus } from "./components/useSheetFocus";
import { ServiceBoard } from "./Services";
import {
  IconSidebar,
  IconChat,
  IconDown,
  IconDatabase,
  IconMenu,
  IconMoon,
  IconPlus,
  IconSliders,
  IconSun,
  IconWhatsApp,
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
  const [whatsapp, setWhatsapp] = useState(false);
  const [rail, setRail] = useState(false);
  const [isMobile, setIsMobile] = useState(() => window.matchMedia("(max-width: 860px)").matches);
  const railRef = useSheetFocus(rail && isMobile, '[data-mobile-menu="true"]');
  const [announcement, setAnnouncement] = useState("");
  const [demo, setDemo] = useState(() => new URLSearchParams(window.location.search).has("demo"));
  const sidebar = useSidebar();
  const { collapsed: railCollapsed, setCollapsed: setRailCollapsed } = sidebar;
  const reducedMotion = useReducedMotion();
  const followReply = useRef(true);
  const [showLatest, setShowLatest] = useState(false);
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

  useEffect(() => {
    const media = window.matchMedia("(max-width: 860px)");
    const update = () => {
      setIsMobile(media.matches);
      if (!media.matches) setRail(false);
    };
    media.addEventListener("change", update);
    return () => media.removeEventListener("change", update);
  }, []);

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
      if (q.has("whatsapp")) setWhatsapp(true);
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
    if (el && followReply.current) el.scrollTop = el.scrollHeight;
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
        setWhatsapp(false);
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

  const send = async () => {
    const text = input.trim();
    if (!text || busy || listening || !snap || !conversationId) return;
    followReply.current = true;
    setShowLatest(false);
    setAnnouncement("");
    setInput("");
    if (boxRef.current) boxRef.current.style.height = "auto";
    setBubbles((b) => [...b, { id: uid(), kind: "user", text }]);
    setBusy(true);
    setReceiving(false);
    const run = ++chatRunRef.current;
    const replyId = uid();
    let replyVisible = false;
    let pendingText = "";
    let fullReply = "";
    let frame: number | null = null;
    const flush = () => {
      frame = null;
      const text = pendingText;
      pendingText = "";
      if (!text || chatRunRef.current !== run) return;
      setReceiving(true);
      if (!replyVisible) {
        replyVisible = true;
        setBubbles((current) => [...current, { id: replyId, kind: "ira", text }]);
      } else {
        setBubbles((current) => current.map((bubble) => bubble.id === replyId ? { ...bubble, text: bubble.text + text } : bubble));
      }
    };
    try {
      await streamChat(conversationId, text, (event) => {
        if (chatRunRef.current !== run) return;
        if (event.type === "delta") {
          if (!event.text) return;
          pendingText += event.text;
          fullReply += event.text;
          if (frame === null) frame = requestAnimationFrame(flush);
        } else if (event.type === "reset") {
          if (frame !== null) cancelAnimationFrame(frame);
          frame = null;
          pendingText = "";
          fullReply = "";
          replyVisible = false;
          setReceiving(false);
          setBubbles((current) => current.filter((bubble) => bubble.id !== replyId));
        }
      });
      if (frame !== null) cancelAnimationFrame(frame);
      flush();
      if (chatRunRef.current === run) setAnnouncement(fullReply ? `Ira: ${fullReply}` : "Ira ha terminado la respuesta.");
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
      if (frame !== null) cancelAnimationFrame(frame);
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
    setDemo(false);
    setAnnouncement("");
    followReply.current = true;
    setShowLatest(false);
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
    setDemo(false);
    setAnnouncement("");
    const run = ++chatRunRef.current;
    setBusy(false);
    setReceiving(false);
    stopVoice();
    try {
      const turns = await openChat(id);
      if (chatRunRef.current !== run) return;
      followReply.current = true;
      setShowLatest(false);
      setConversationId(id);
      setBubbles(turnsToBubbles(turns));
      setRail(false);
    } catch (error) {
      if (chatRunRef.current === run) setBoot(error instanceof Error ? error.message : String(error));
    }
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
    else setRailCollapsed(!railCollapsed);
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
  const chatting = bubbles.length > 0 || busy || listening;
  const canSend = Boolean(snap) && Boolean(conversationId) && !busy && !listening && input.trim().length > 0;
  const canTalk = Boolean(snap) && Boolean(conversationId) && (listening || !busy);

  const changeMode = (op: Op) => {
    void onOp(op).catch((error) => setBoot(error instanceof Error ? error.message : String(error)));
  };
  const composer = <Composer input={input} onInput={setInput} boxRef={boxRef} snap={snap}
    busy={busy} listening={listening} canTalk={canTalk} canSend={canSend}
    onSend={() => void send()} onTalk={() => void talk()}
    onModel={(id) => { if (id) changeMode({ op: "activate_model", id }); }}
    onThinking={() => changeMode({ op: "set_thinking", value: !thinking })}
    onTools={() => changeMode({ op: "set_tools_enabled", value: !useTools })} />;

  return (
    <MotionConfig reducedMotion="user" transition={{ type: "spring", stiffness: 380, damping: 36 }}>
    <div style={{ "--rail-width": `${sidebar.width}px` } as CSSProperties} className={`app${rail ? " rail-open" : ""}${railCollapsed ? " rail-collapsed" : ""}${sidebar.dragging ? " rail-resizing" : ""}${catalog || databases || voiceOpen ? " sheet-open" : ""}`}>
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

      <aside ref={railRef} className="rail" aria-label="navegación" role={isMobile && rail ? "dialog" : undefined} aria-modal={isMobile && rail ? true : undefined} inert={catalog || databases || whatsapp || voiceOpen}>
        <div className="rail-top">
          <p className="brand">
             <img src="/ira-cabeza-recortada.png" alt="" />
            <span className="rail-label">Ira<span className="brand-caption">Tu espacio de inteligencia</span></span>
          </p>
          <button
            type="button"
            className="btn-ghost rail-close"
            data-desktop-menu="true"
            aria-label={isMobile ? "Cerrar menú" : railCollapsed ? "Ampliar barra lateral" : "Compactar barra lateral"}
            aria-expanded={isMobile ? rail : !railCollapsed}
            title={isMobile ? "Cerrar menú" : railCollapsed ? "Ampliar barra lateral" : "Compactar barra lateral"}
            onClick={closeRail}
          >
            <IconSidebar />
          </button>
        </div>

        <button type="button" className="btn-primary new-chat" title="Nueva conversación" aria-label="Nueva conversación" onClick={() => void clear()}>
          <IconPlus />
          <span className="rail-label">Nueva conversación</span>
        </button>

        <nav className="rail-chats" aria-label="conversaciones">
          <p className="rail-section-label rail-label">Conversaciones</p>
          {chats.length === 0 && <p className="rail-empty rail-label">Tu próxima idea empieza aquí.</p>}
          {chats.map((c) => (
            <button
              key={c.id}
              type="button"
              className={`chat-item${c.id === conversationId ? " on" : ""}`}
              title={c.title?.trim() || "Nuevo chat"}
              aria-label={c.title?.trim() || "Nuevo chat"}
              aria-current={c.id === conversationId ? "page" : undefined}
              onClick={() => void open(c.id)}
            >
              {c.id === conversationId && <motion.span className="nav-selection" layoutId="chat-selection" transition={{ type: "spring", stiffness: 420, damping: 38 }} />}
              <IconChat /><span className="rail-label">{c.title?.trim() || "Nuevo chat"}</span>
            </button>
          ))}
        </nav>

        <nav className="rail-nav">
          <button
            type="button"
            className={`nav-item${catalog ? " on" : ""}`}
            data-sheet-trigger={catalog ? "true" : undefined}
            title="Modelos y configuración" aria-label="Modelos y configuración"
            onClick={() => { setCatalog(true); setDatabases(false); setWhatsapp(false); setRail(false); }}
          >
            <IconSliders />
            <span className="rail-label">Modelos y configuración</span>
          </button>
          <button
            type="button"
            className={`nav-item${databases ? " on" : ""}`}
            data-sheet-trigger={databases ? "true" : undefined}
            title="Bases de datos" aria-label="Bases de datos"
            onClick={() => { setDatabases(true); setCatalog(false); setWhatsapp(false); setRail(false); }}
          >
            <IconDatabase />
            <span className="rail-label">Bases de datos</span>
          </button>
          <button
            type="button"
            className={`nav-item${whatsapp ? " on" : ""}`}
            data-sheet-trigger={whatsapp ? "true" : undefined}
            title="WhatsApp" aria-label="WhatsApp"
            onClick={() => { setWhatsapp(true); setCatalog(false); setDatabases(false); setRail(false); }}
          >
            <IconWhatsApp />
            <span className="rail-label">WhatsApp</span>
          </button>
        </nav>

        <ServiceBoard
          data={services}
          starting={starting}
          compact
          onStart={() => void bootServices()}
        />

        <div className="rail-foot">
          <button type="button" className="btn-ghost theme-btn" onClick={toggleTheme} title={theme === "dark" ? "Modo claro" : "Modo oscuro"} aria-label={theme === "dark" ? "Modo claro" : "Modo oscuro"}>
            {theme === "dark" ? <IconSun /> : <IconMoon />}
            <span className="rail-label">{theme === "dark" ? "Modo claro" : "Modo oscuro"}</span>
          </button>
        </div>
        {!railCollapsed && <div className="rail-resize" {...sidebar.resizeProps} />}
      </aside>

      <div className="stage" inert={catalog || databases || whatsapp || voiceOpen || rail}>
        <header className="topbar">
          <button
            type="button"
            className="btn-ghost menu-btn"
            data-mobile-menu="true"
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
               <img className="hero-logo" src="/ira-cabeza-recortada.png" alt="" />
              <h1>Una idea. Infinitas posibilidades.</h1>
              <p>
                {boot
                  ? boot
                  : "Piensa, pregunta, conecta. Hagámoslo juntos."}
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
            <div className="log" ref={listRef} aria-label="Conversación" onScroll={(event) => {
              const el = event.currentTarget;
              followReply.current = el.scrollHeight - el.scrollTop - el.clientHeight < 96;
              setShowLatest(!followReply.current);
            }}>
              {demo && <p className="demo-notice" role="status">Conversación de ejemplo · datos simulados</p>}
              {boot && <p className="bubble error" role="alert">{boot}</p>}
              <AnimatePresence initial={false}>
              {bubbles.map((b, index) => <Message key={b.id} bubble={b} streaming={busy && receiving && b.kind === "ira" && index === bubbles.length - 1} />)}
              </AnimatePresence>
              <AnimatePresence>
              {busy && !receiving && (
                <motion.article
                  className="turn ira"
                  aria-live="polite"
                  initial={reducedMotion ? false : { y: 8 }}
                  animate={{ y: 0 }}
                  exit={{ opacity: 0, transition: { duration: 0.08 } }}
                >
                   <span className="who"><img src="/ira-cabeza-recortada.png" alt="" /></span>
                  <div className="bubble ira load">
                    <Activity />
                    {thinking ? "Razonando tu respuesta" : "Preparando tu respuesta"}
                  </div>
                </motion.article>
              )}
              </AnimatePresence>
            </div>
          )}

          <motion.div className="dock" layout={reducedMotion ? false : "position"}>
            {showLatest && chatting && <button type="button" className="latest-button" onClick={() => {
              followReply.current = true;
              setShowLatest(false);
              listRef.current?.scrollTo({ top: listRef.current.scrollHeight, behavior: "instant" });
            }}><IconDown />Ir al último mensaje</button>}
            {composer}
            <p className="composer-hint">{chatting ? "Ira puede equivocarse. Comprueba la información importante." : "Enter para enviar · Shift + Enter para una nueva línea"}</p>
          </motion.div>
        </div>
      </div>

      <div className="sr-only" role="status" aria-live="polite" aria-atomic="true">{announcement}</div>
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
      {whatsapp && (
          <motion.button type="button" className="scrim settings" aria-label="cerrar WhatsApp" initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }} onClick={() => setWhatsapp(false)} />
      )}
      </AnimatePresence>
      <AnimatePresence>
      {whatsapp && (
          <WhatsApp onClose={() => setWhatsapp(false)} />
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
