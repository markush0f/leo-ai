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
  renameChat,
  deleteChat,
  deleteAllChats,
  loadServices,
  setService,
  startServices,
  updateWebSearch,
  loadSnapshot,
  newChat,
  openChat,
  streamChat,

} from "./api";
import { Catalog } from "./Catalog";
import { Databases } from "./Databases";
import { DocsSheet } from "./Docs";
import { Message } from "./components/Message";
import { Composer } from "./components/Composer";
import { Activity } from "./components/Activity";
import { useSidebar } from "./components/useSidebar";
import { useSheetFocus } from "./components/useSheetFocus";
import { ServicesSheet } from "./Services";
import { btn, cx, scrim, scrimSettings } from "./ui";
import {
  IconSidebar,
  IconChat,
  IconDown,
  IconDatabase,
  IconMenu,
  IconMoon,
  IconPlus,
  IconPower,
  IconSliders,
  IconSun,
  IconTools,
  IconBook,
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

export default function App({ onLogout }: { onLogout?: () => void }) {
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
  const [docs, setDocs] = useState(false);
  const [servicesOpen, setServicesOpen] = useState(false);
  const [servicesTab, setServicesTab] = useState<"service" | "mcp">("service");
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
  const [svcBusy, setSvcBusy] = useState<string | null>(null);
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
    const id = window.setInterval(() => {
      void loadServices()
        .then(setServices)
        .catch(() => undefined);
    }, 4000);
    return () => window.clearInterval(id);
  }, []);

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
        setDocs(false);
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

  const onWebSearchChange = async (enabled: boolean, contextSize: string) => {
    const preferences = await updateWebSearch(enabled, contextSize);
    setSnap((current) => current && ({
      ...current,
      web_search_enabled: preferences.web_search_enabled,
      web_search_context_size: preferences.web_search_context_size,
    }));
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
        } else if (event.type === "mcp_used") {
          setBubbles((current) => {
            const bubble = current.find((item) => item.id === replyId);
            const names = new Set(bubble?.mcps ?? []);
            names.add(event.tool_name);
            return bubble
              ? current.map((item) => item.id === replyId ? { ...item, mcps: [...names] } : item)
              : [...current, { id: replyId, kind: "ira", text: "", mcps: [...names] }];
          });
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

  const renameConversation = async (chat: Conversation) => {
    const title = window.prompt("Nombre de la conversación", chat.title?.trim() || "Nuevo chat");
    if (title === null || !title.trim()) return;
    try { setChats(await renameChat(chat.id, title)); }
    catch (error) { setBoot(error instanceof Error ? error.message : String(error)); }
  };

  const removeConversation = async (chat: Conversation) => {
    if (!window.confirm(`¿Eliminar «${chat.title?.trim() || "Nuevo chat"}»?`)) return;
    try {
      const next = await deleteChat(chat.id);
      setChats(next);
      if (chat.id === conversationId && next[0]) await open(next[0].id);
    } catch (error) { setBoot(error instanceof Error ? error.message : String(error)); }
  };

  const removeAllConversations = async () => {
    if (!window.confirm("¿Eliminar todas las conversaciones?")) return;
    try {
      const next = await deleteAllChats();
      setChats(next);
      if (next[0]) await open(next[0].id);
    } catch (error) { setBoot(error instanceof Error ? error.message : String(error)); }
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

  const configureService = async (
    id: string,
    action: "autostart" | "manual" | "port" | "boot" | "meta",
    port?: number,
    name?: string,
    description?: string,
  ) => {
    setSvcBusy(id || "boot");
    try {
      const next = action === "boot" ? await startServices() : await setService(id, action, port, name, description);
      setServices(next);
      if (next.error) setBoot(next.error);
      if (action === "port" && id === "postgres") await refresh();
    } catch (e) {
      setBoot(e instanceof Error ? e.message : String(e));
    } finally {
      setSvcBusy(null);
    }
  };

  const toggleService = async (id: string, running: boolean) => {
    setSvcBusy(id);
    try {
      const next = await setService(id, running ? "stop" : "start");
      setServices(next);
      if (next.error) setBoot(next.error);
      if (id === "postgres" && !running) {
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
      }
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      setBoot(msg);
      setServices((prev) => ({
        ok: false,
        services: prev?.services ?? [],
        gateway_port: prev?.gateway_port,
        error: msg,
      }));
    } finally {
      setSvcBusy(null);
    }
  };

  const useTools = snap?.tools_enabled ?? true;
  const useMutate = snap?.tools_mutate ?? false;
  const model = snap?.models.find((m) => m.id === snap.active_model_id);
  const effort = model?.effort || "low";
  const thinking = effort !== "low";
  const provider = snap?.providers.find((p) => p.id === model?.provider_id);
  const chatting = bubbles.length > 0 || busy || listening;
  const catalogOpen = catalog && snap !== null;
  const canSend = Boolean(snap) && Boolean(conversationId) && !busy && !listening && input.trim().length > 0;
  const canTalk = Boolean(snap) && Boolean(conversationId) && (listening || !busy);

  const changeMode = (op: Op) => {
    void onOp(op).catch((error) => setBoot(error instanceof Error ? error.message : String(error)));
  };
  const composer = <Composer input={input} onInput={setInput} boxRef={boxRef} snap={snap}
    busy={busy} listening={listening} canTalk={canTalk} canSend={canSend}
    onSend={() => void send()} onTalk={() => void talk()}
    onModel={(id) => { if (id) changeMode({ op: "activate_model", id }); }}
    onEffort={(id, effort) => changeMode({ op: "set_model_effort", id, effort })}
    onTools={() => changeMode({ op: "set_tools_enabled", value: !useTools })}
    onMutate={() => changeMode({ op: "set_tools_mutate", value: !useMutate })} />;

  return (
    <MotionConfig reducedMotion="user" transition={{ type: "spring", stiffness: 380, damping: 36 }}>
    <div style={{ "--rail-width": `${sidebar.width}px` } as CSSProperties} className={cx("app flex h-full overflow-hidden bg-bg", rail && "rail-open", railCollapsed && "rail-collapsed", sidebar.dragging && "rail-resizing cursor-col-resize select-none", (catalogOpen || databases || docs || servicesOpen || voiceOpen) && "sheet-open")}>
      <AnimatePresence>
      {rail && (
        <motion.button
          type="button"
          className={cx(scrim, "mobile:block")}
          aria-label="cerrar menú"
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          exit={{ opacity: 0 }}
          onClick={() => setRail(false)}
        />
      )}
      </AnimatePresence>

      <aside ref={railRef} className={cx("rail relative z-[4] flex w-[var(--rail-width,272px)] shrink-0 flex-col gap-3 border-r border-line bg-sidebar px-[0.9rem] pt-[1.2rem] pb-4 transition-[width,padding] duration-[420ms] ease-[cubic-bezier(0.22,1,0.36,1)] collapsed:!w-[76px] collapsed:!px-3 dragging:!transition-none mobile:fixed mobile:inset-y-0 mobile:left-0 mobile:z-[6] mobile:w-[min(310px,calc(100vw-48px))] mobile:-translate-x-[105%] mobile:transition-transform mobile:duration-200 mobile:ease-[ease] mobile:pt-[max(1.2rem,env(safe-area-inset-top))] mobile:pb-[max(1rem,env(safe-area-inset-bottom))]", isMobile && !rail && "invisible", isMobile && rail && "visible !translate-x-0")} aria-label="navegación" role={isMobile && rail ? "dialog" : undefined} aria-modal={isMobile && rail ? true : undefined} inert={catalogOpen || databases || docs || servicesOpen || voiceOpen}>
        <div className="flex items-center justify-between gap-[0.3rem] px-1 pt-[0.15rem] pb-[0.35rem] desk:relative desk:h-[54px] desk:shrink-0 desk:p-0 desk:transition-[height] desk:duration-[420ms] desk:ease-[cubic-bezier(0.22,1,0.36,1)] collapsed:!h-[106px]">
          <p className="m-0 flex items-center gap-[0.65rem] text-[1.25rem] font-[650] tracking-[-0.03em] [&_img]:h-[3.2rem] [&_img]:w-[2.8rem] [&_img]:object-contain desk:absolute desk:top-1/2 desk:left-1 desk:-translate-y-1/2 desk:transition-[left,top,transform] desk:duration-[420ms] desk:ease-[cubic-bezier(0.22,1,0.36,1)] collapsed:!top-[26px] collapsed:!left-1/2 collapsed:!-translate-x-1/2 collapsed:!-translate-y-1/2">
             <img src="/ira-cabeza-recortada.png" alt="" />
            <span className="min-w-0 truncate collapsed:!hidden">Ira<span className="mt-[0.15rem] block text-[0.68rem] font-[450] tracking-normal text-muted">Tu espacio de inteligencia</span></span>
          </p>
          <button
            type="button"
            className={cx(btn.ghost, "shrink-0 !p-2 desk:absolute desk:top-1/2 desk:right-1 desk:-translate-y-1/2 desk:transition-[right,top,transform] desk:duration-[420ms] desk:ease-[cubic-bezier(0.22,1,0.36,1)] collapsed:!top-[79px] collapsed:!right-1/2 collapsed:!translate-x-1/2 collapsed:!-translate-y-1/2")}
            data-desktop-menu="true"
            aria-label={isMobile ? "Cerrar menú" : railCollapsed ? "Ampliar barra lateral" : "Compactar barra lateral"}
            aria-expanded={isMobile ? rail : !railCollapsed}
            title={isMobile ? "Cerrar menú" : railCollapsed ? "Ampliar barra lateral" : "Compactar barra lateral"}
            onClick={closeRail}
          >
            <IconSidebar />
          </button>
        </div>

        <button type="button" className={cx(btn.primary, "mt-[0.65rem] min-h-11 w-full justify-start px-[0.85rem] collapsed:!justify-center collapsed:!px-0")} title="Nueva conversación" aria-label="Nueva conversación" onClick={() => void clear()}>
          <IconPlus />
          <span className="min-w-0 truncate collapsed:!hidden">Nueva conversación</span>
        </button>

        <nav className="flex min-h-0 flex-1 flex-col gap-[0.15rem] overflow-auto pr-[0.1rem] transition-[flex-grow,opacity,visibility] duration-[420ms] ease-[cubic-bezier(0.22,1,0.36,1)] collapsed:!grow-0 collapsed:!opacity-0 collapsed:!invisible collapsed:![transition:flex-grow_420ms_cubic-bezier(0.22,1,0.36,1),opacity_180ms_ease,visibility_0s_180ms]" aria-label="conversaciones">
          <div className="mt-[1.15rem] mb-[0.6rem] flex items-center justify-between px-[0.7rem] collapsed:!hidden">
            <p className="m-0 truncate text-[0.78rem] font-semibold text-muted">Conversaciones</p>
             {chats.length > 0 && <button type="button" className="min-h-11 px-2 text-xs text-muted hover:text-ink" title="Eliminar todas las conversaciones" aria-label="Eliminar todas las conversaciones" onClick={() => void removeAllConversations()}>Eliminar todo</button>}
          </div>
          {chats.length === 0 && <p className="px-[0.7rem] text-[0.82rem] whitespace-normal text-muted collapsed:!hidden">Tu próxima idea empieza aquí.</p>}
          {chats.map((c) => (
             <div key={c.id} className={cx("group relative flex min-h-11 w-full items-center rounded-[10px] hover:bg-elevated", c.id === conversationId && "bg-[color-mix(in_srgb,var(--color-accent)_12%,var(--color-sidebar))]")}>
               <button type="button" className="flex min-h-11 min-w-0 flex-1 items-center gap-[0.65rem] overflow-hidden rounded-[10px] border-0 bg-transparent px-[0.7rem] py-[0.45rem] text-left font-medium text-ink collapsed:!justify-center collapsed:!px-0" title={c.title?.trim() || "Nuevo chat"} aria-label={c.title?.trim() || "Nuevo chat"} aria-current={c.id === conversationId ? "page" : undefined} onClick={() => void open(c.id)}>
                <IconChat /><span className="min-w-0 truncate collapsed:!hidden">{c.title?.trim() || "Nuevo chat"}</span>
              </button>
              <div className="flex shrink-0 items-center pr-1 collapsed:!hidden">
                 <button type="button" className="flex size-11 items-center justify-center rounded text-muted hover:text-ink" title="Editar nombre" aria-label={`Editar nombre: ${c.title || "Nuevo chat"}`} onClick={() => void renameConversation(c)}>✎</button>
                 <button type="button" className="flex size-11 items-center justify-center rounded text-muted hover:text-danger" title="Eliminar conversación" aria-label={`Eliminar: ${c.title || "Nuevo chat"}`} onClick={() => void removeConversation(c)}>×</button>
              </div>
            </div>
          ))}
        </nav>

         <nav className="flex shrink-0 flex-col gap-1 overflow-y-auto border-t border-line pt-3 mobile:max-h-[min(45vh,19rem)]">
          <button
            type="button"
            className={cx("flex min-h-11 w-full items-center gap-[0.6rem] rounded-[10px] border-0 bg-transparent px-[0.7rem] py-[0.55rem] text-left font-[550] text-ink hover:bg-elevated [&_svg]:size-[18px] collapsed:!justify-center collapsed:!px-0", catalog && "bg-elevated")}
            data-sheet-trigger={catalog ? "true" : undefined}
            title="Modelos y configuración" aria-label="Modelos y configuración"
            onClick={() => {
              if (!snap) {
                setBoot("No se pudo cargar la configuración. Comprueba la conexión e inténtalo de nuevo.");
                setRail(false);
                return;
              }
              setCatalog(true); setDatabases(false); setServicesOpen(false); setDocs(false); setRail(false);
            }}
          >
            <IconSliders />
            <span className="min-w-0 truncate collapsed:!hidden">Modelos y configuración</span>
          </button>
          <button
            type="button"
            className={cx("flex min-h-11 w-full items-center gap-[0.6rem] rounded-[10px] border-0 bg-transparent px-[0.7rem] py-[0.55rem] text-left font-[550] text-ink hover:bg-elevated [&_svg]:size-[18px] collapsed:!justify-center collapsed:!px-0", databases && "bg-elevated")}
            data-sheet-trigger={databases ? "true" : undefined}
            title="Bases de datos" aria-label="Bases de datos"
            onClick={() => { setDatabases(true); setCatalog(false); setServicesOpen(false); setDocs(false); setRail(false); }}
          >
            <IconDatabase />
            <span className="min-w-0 truncate collapsed:!hidden">Bases de datos</span>
          </button>
          <button
            type="button"
            className={cx("flex min-h-11 w-full items-center gap-[0.6rem] rounded-[10px] border-0 bg-transparent px-[0.7rem] py-[0.55rem] text-left font-[550] text-ink hover:bg-elevated [&_svg]:size-[18px] collapsed:!justify-center collapsed:!px-0", servicesOpen && servicesTab === "service" && "bg-elevated")}
            data-sheet-trigger={servicesOpen && servicesTab === "service" ? "true" : undefined}
            title="Servicios" aria-label="Servicios"
            onClick={() => { setServicesTab("service"); setServicesOpen(true); setCatalog(false); setDatabases(false); setDocs(false); setRail(false); }}
          >
            <IconPower />
            <span className="min-w-0 truncate collapsed:!hidden">Servicios</span>
          </button>
          <button
            type="button"
            className={cx("flex min-h-11 w-full items-center gap-[0.6rem] rounded-[10px] border-0 bg-transparent px-[0.7rem] py-[0.55rem] text-left font-[550] text-ink hover:bg-elevated [&_svg]:size-[18px] collapsed:!justify-center collapsed:!px-0", servicesOpen && servicesTab === "mcp" && "bg-elevated")}
            data-sheet-trigger={servicesOpen && servicesTab === "mcp" ? "true" : undefined}
            title="MCP" aria-label="MCP"
            onClick={() => { setServicesTab("mcp"); setServicesOpen(true); setCatalog(false); setDatabases(false); setDocs(false); setRail(false); }}
          >
            <IconTools />
            <span className="min-w-0 truncate collapsed:!hidden">MCP</span>
          </button>
          <button
            type="button"
            className={cx("flex min-h-11 w-full items-center gap-[0.6rem] rounded-[10px] border-0 bg-transparent px-[0.7rem] py-[0.55rem] text-left font-[550] text-ink hover:bg-elevated [&_svg]:size-[18px] collapsed:!justify-center collapsed:!px-0", docs && "bg-elevated")}
            data-sheet-trigger={docs ? "true" : undefined}
            title="Documentación" aria-label="Documentación"
            onClick={() => { setDocs(true); setCatalog(false); setDatabases(false); setServicesOpen(false); setRail(false); }}
          >
            <IconBook />
            <span className="min-w-0 truncate collapsed:!hidden">Documentación</span>
          </button>
        </nav>

        <div className="mt-auto flex flex-col gap-[0.4rem] collapsed:!mt-0">
          {onLogout && <button type="button" className={cx(btn.ghost, "w-full justify-start collapsed:!justify-center")} onClick={onLogout} title="Cerrar sesión" aria-label="Cerrar sesión">
            <span className="collapsed:!hidden">Cerrar sesión</span>
            <span className="hidden collapsed:!inline">Salir</span>
          </button>}
          <button type="button" className={cx(btn.ghost, "w-full justify-start gap-[0.55rem] collapsed:!justify-center collapsed:!px-0")} onClick={toggleTheme} title={theme === "dark" ? "Modo claro" : "Modo oscuro"} aria-label={theme === "dark" ? "Modo claro" : "Modo oscuro"}>
            {theme === "dark" ? <IconSun /> : <IconMoon />}
            <span className="min-w-0 truncate collapsed:!hidden">{theme === "dark" ? "Modo claro" : "Modo oscuro"}</span>
          </button>
        </div>
        {!railCollapsed && <div className="absolute inset-y-0 -right-1 z-[5] w-[9px] cursor-col-resize touch-none after:absolute after:top-[42%] after:bottom-[42%] after:left-1 after:w-0.5 after:rounded-sm after:bg-transparent after:transition-colors after:duration-150 hover:after:bg-accent focus-visible:after:bg-accent dragging:after:bg-accent mobile:hidden" {...sidebar.resizeProps} />}
      </aside>

      <div className="stage flex min-w-0 flex-1 flex-col bg-bg" inert={catalogOpen || databases || docs || servicesOpen || voiceOpen || rail}>
         <header className="flex min-h-[76px] items-center gap-2 border-b border-[color-mix(in_srgb,var(--color-line)_55%,transparent)] px-[1.8rem] py-4 mobile:min-h-16 mobile:px-4 mobile:py-3 mobile:pt-[max(0.75rem,env(safe-area-inset-top))]">
          <button
            type="button"
            className={cx(btn.ghost, "!hidden mobile:!inline-flex")}
            data-mobile-menu="true"
            aria-label="mostrar barra lateral"
            title="Mostrar barra lateral"
            onClick={openRail}
          >
            <IconMenu />
          </button>
          <p className="m-0 mr-auto ml-1 text-[0.95rem] font-semibold phone:min-w-0 phone:truncate [&_span]:text-[0.83rem] [&_span]:font-medium [&_span]:text-muted phone:[&_span]:hidden">
            {model ? `${model.name}` : "Ira"}
            {provider ? <span> · {provider.name}</span> : null}
          </p>
          <button
            type="button"
            className={cx(btn.ghost, "w-auto !p-[0.4rem]")}
            onClick={toggleTheme}
            aria-label={theme === "dark" ? "modo claro" : "modo oscuro"}
          >
            {theme === "dark" ? <IconSun /> : <IconMoon />}
          </button>
        </header>

        <div className={chatting ? "relative flex min-h-0 flex-1 flex-col" : "welcome relative flex min-h-0 flex-1 flex-col items-center justify-center gap-8 px-5 pt-4 pb-[6vh] mobile:px-2 mobile:pt-4 mobile:pb-8 phone:justify-[safe_center] phone:gap-6 phone:overflow-y-auto short:justify-[safe_center] short:gap-[1.1rem] short:overflow-y-auto short:pb-4"}>
          {!chatting ? (
            <div className="w-[min(38rem,100%)] text-center">
               <img className="hero-logo mx-auto mb-[1.6rem] block size-[132px] animate-logo object-contain phone:mb-4 phone:size-[108px] short:mb-[0.8rem] short:h-[70px] short:w-16" src="/ira-cabeza-recortada.png" alt="" />
              <h1 className="mx-auto m-0 max-w-[16ch] text-[clamp(1.9rem,3.2vw,2.9rem)] leading-[1.12] font-[620] tracking-[-0.035em] text-balance mobile:text-[clamp(1.9rem,5vw,2.6rem)]">Una idea. Infinitas posibilidades.</h1>
              <p className="mt-4 text-base text-muted phone:px-4 phone:text-[0.95rem]">
                {boot
                  ? boot
                  : "Piensa, pregunta, conecta. Hagámoslo juntos."}
              </p>

            </div>
          ) : (
            <div className="log min-h-0 flex-1 overflow-auto pt-8 pb-6 [overflow-anchor:none] [overscroll-behavior:contain]" ref={listRef} aria-label="Conversación" onScroll={(event) => {
              const el = event.currentTarget;
              followReply.current = el.scrollHeight - el.scrollTop - el.clientHeight < 96;
              setShowLatest(!followReply.current);
            }}>
              {demo && <p className="mx-auto mb-6 max-w-[47rem] px-6 text-[0.8rem] text-muted" role="status">Conversación de ejemplo · datos simulados</p>}
              {boot && <p className="mx-6 rounded-xl border border-[color-mix(in_srgb,var(--color-danger)_45%,var(--color-line))] bg-[color-mix(in_srgb,var(--color-danger)_5%,var(--color-surface))] p-4 text-danger" role="alert">{boot}</p>}
              <AnimatePresence initial={false}>
              {bubbles.map((b, index) => <Message key={b.id} bubble={b} streaming={busy && receiving && b.kind === "ira" && index === bubbles.length - 1} />)}
              </AnimatePresence>
              <AnimatePresence>
              {busy && !receiving && (
                <motion.article
                  className="turn ira mx-auto mb-8 grid w-full max-w-[50rem] grid-cols-[34px_minmax(0,1fr)] items-start gap-x-3 px-6 phone:px-4"
                  aria-live="polite"
                  initial={reducedMotion ? false : { y: 8 }}
                  animate={{ y: 0 }}
                  exit={{ opacity: 0, transition: { duration: 0.08 } }}
                >
                   <span className="m-0 flex items-center [&_img]:size-[34px] [&_img]:object-contain"><img src="/ira-cabeza-recortada.png" alt="" /></span>
                  <div className="flex min-w-0 items-center gap-2 pl-0 leading-[1.7] text-muted">
                    <Activity />
                    {thinking ? `Razonando · ${effort}` : "Preparando tu respuesta"}
                  </div>
                </motion.article>
              )}
              </AnimatePresence>
            </div>
          )}

           <motion.div className="relative mx-auto w-full max-w-[50rem] px-6 pb-4 welcome:max-w-[48rem] phone:px-4 phone:pb-[max(0.6rem,env(safe-area-inset-bottom))]" layout={reducedMotion ? false : "position"}>
            {showLatest && chatting && <button type="button" className="absolute bottom-[calc(100%+12px)] left-1/2 flex -translate-x-1/2 items-center gap-[0.4rem] rounded-full border border-line bg-surface px-[0.9rem] py-[0.55rem] text-[0.8rem] whitespace-nowrap text-ink [&_svg]:size-4" onClick={() => {
              followReply.current = true;
              setShowLatest(false);
              listRef.current?.scrollTo({ top: listRef.current.scrollHeight, behavior: "instant" });
            }}><IconDown />Ir al último mensaje</button>}
            {composer}
            <p className="mt-[0.65rem] text-center text-[0.73rem] text-muted phone:px-1 phone:text-[0.67rem]">{chatting ? "Ira puede equivocarse. Comprueba la información importante." : "Enter para enviar · Shift + Enter para una nueva línea"}</p>
          </motion.div>
        </div>
      </div>

      <div className="sr-only" role="status" aria-live="polite" aria-atomic="true">{announcement}</div>
      <AnimatePresence>
      {catalogOpen && snap && (
          <motion.button
            type="button"
            className={scrimSettings}
            aria-label="cerrar catálogo"
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            onClick={() => setCatalog(false)}
          />
      )}
      </AnimatePresence>
      <AnimatePresence>
      {catalogOpen && snap && (
          <Catalog
            snap={snap}
            onOp={onOp}
            onWebSearchChange={onWebSearchChange}
            onCodexLogin={onCodexLogin}
            onClose={() => setCatalog(false)}
          />
      )}
      </AnimatePresence>
      <AnimatePresence>
      {databases && (
          <motion.button type="button" className={scrimSettings} aria-label="cerrar bases de datos" initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }} onClick={() => setDatabases(false)} />
      )}
      </AnimatePresence>
      <AnimatePresence>
      {databases && (
          <Databases onClose={() => setDatabases(false)} />
      )}
      </AnimatePresence>
      <AnimatePresence>
      {docs && (
          <motion.button type="button" className={scrimSettings} aria-label="cerrar documentación" initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }} onClick={() => setDocs(false)} />
      )}
      </AnimatePresence>
      <AnimatePresence>
      {docs && (
          <DocsSheet onClose={() => setDocs(false)} />
      )}
      </AnimatePresence>
      <AnimatePresence>
      {servicesOpen && (
          <motion.button type="button" className={scrimSettings} aria-label="cerrar servicios" initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }} onClick={() => setServicesOpen(false)} />
      )}
      </AnimatePresence>
      <AnimatePresence>
      {servicesOpen && (
          <ServicesSheet
            tab={servicesTab}
            onTab={setServicesTab}
            data={services}
            busyId={svcBusy}
            onClose={() => setServicesOpen(false)}
            onToggle={(id, running) => void toggleService(id, running)}
            onBoot={(id, on) => void configureService(id, on ? "autostart" : "manual")}
            onPort={(id, port) => void configureService(id, "port", port)}
            onMeta={(id, name, description) => void configureService(id, "meta", undefined, name, description)}
            onStartSelected={() => void configureService("", "boot")}
          />
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
