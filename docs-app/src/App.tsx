/**
 * Standalone documentation reader for the Ira workspace. Articles are plain
 * Markdown files compiled into the bundle; language and theme are persisted
 * preferences with no server dependency.
 */
import { useEffect, useMemo, useRef, useState, type ComponentProps } from "react";
import ReactMarkdown, { defaultUrlTransform } from "react-markdown";
import remarkGfm from "remark-gfm";
import { brand, card, cx, doc, iconBtn, lang as langToggle, langOn, md, menuToggle, scrim, search, shell, sidebar, tocHead, tocHeadOpen, tocItem, tocItemOn, topbar } from "./ui";
import {
  articles,
  articleBody,
  sectionOf,
  sections,
  type Article,
  type Lang,
} from "./content";

const LANG_KEY = "ira-docs-lang";
const THEME_KEY = "ira-docs-theme";

type Theme = "light" | "dark";

function readLang(): Lang {
  const stored = localStorage.getItem(LANG_KEY);
  if (stored === "en" || stored === "es") return stored;
  return navigator.language.toLowerCase().startsWith("es") ? "es" : "en";
}

function readTheme(): Theme {
  const stored = localStorage.getItem(THEME_KEY);
  return stored === "light" || stored === "dark" ? stored : "dark";
}

function currentHashSlug(): string | null {
  const raw = window.location.hash.replace(/^#\/?/, "");
  return raw ? decodeURIComponent(raw) : null;
}

/** First ±60-char excerpt of the body around the search needle, cleaned of Markdown syntax. */
function excerpt(body: string, needle: string): string | null {
  const plain = body.replace(/[#`*_\[\]()|>-]/g, " ").replace(/\s+/g, " ");
  const i = plain.toLowerCase().indexOf(needle);
  if (i < 0) return null;
  const start = Math.max(0, i - 40);
  const end = Math.min(plain.length, i + needle.length + 60);
  return `${start > 0 ? "…" : ""}${plain.slice(start, end).trim()}${end < plain.length ? "…" : ""}`;
}

/**
 * Splits a Markdown article into its `##` sections. The preamble (H1 and any
 * intro before the first `##`) renders expanded; each section is a separate
 * part that opens downward.
 */
function splitParts(text: string): { intro: string; parts: { title: string; body: string }[] } {
  const chunks = text.split(/^## (.*)\n/m);
  const intro = chunks[0] ?? "";
  const parts: { title: string; body: string }[] = [];
  for (let i = 1; i < chunks.length; i += 2) {
    parts.push({ title: chunks[i] ?? "", body: chunks[i + 1] ?? "" });
  }
  return { intro, parts };
}

type Hit = { article: Article; excerpt: string | null };

function searchHits(query: string, lang: Lang): Hit[] {
  const needle = query.trim().toLowerCase();
  if (!needle) return [];
  const scored: { hit: Hit; score: number }[] = [];
  for (const a of articles) {
    const title = a.title[lang].toLowerCase();
    const bodyText = articleBody(lang, a.slug);
    const inTitle = title.includes(needle);
    const inBody = bodyText.toLowerCase().includes(needle);
    if (!inTitle && !inBody) continue;
    scored.push({
      hit: { article: a, excerpt: inBody ? excerpt(bodyText, needle) : a.summary[lang] },
      score: (inTitle ? 0 : 10) + (inBody ? 0 : -5) + sections.findIndex((s) => s.articles.includes(a)),
    });
  }
  scored.sort((x, y) => x.score - y.score);
  return scored.slice(0, 12).map((s) => s.hit);
}

function SearchModal({
  lang,
  onClose,
  onSelect,
}: {
  lang: Lang;
  onClose: () => void;
  onSelect: (slug: string) => void;
}) {
  const [q, setQ] = useState("");
  const [cursor, setCursor] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);
  useEffect(() => inputRef.current?.focus(), []);
  const hits = useMemo(() => searchHits(q, lang), [q, lang]);

  const run = (hit: Hit | undefined) => {
    if (hit) onSelect(hit.article.slug);
  };

  return (
    <div className="fixed inset-0 z-50 flex items-start justify-center bg-black/50 pt-[12vh]" onClick={onClose}>
      <div
        role="dialog"
        aria-modal="true"
        aria-label={lang === "es" ? "Buscar en la documentación" : "Search the documentation"}
        className="mx-4 flex max-h-[65vh] w-full max-w-[560px] flex-col overflow-hidden rounded-2xl border border-line bg-raised shadow-[0_24px_80px_rgba(0,0,0,0.4)]"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="flex items-center gap-2 border-b border-line px-4">
          <span className="text-dim" aria-hidden>⌕</span>
          <input
            ref={inputRef}
            className="w-full bg-transparent py-3.5 text-ink outline-none"
            type="search"
            placeholder={lang === "es" ? "Buscar en toda la documentación…" : "Search all documentation…"}
            value={q}
            onChange={(e) => {
              setQ(e.target.value);
              setCursor(0);
            }}
            onKeyDown={(e) => {
              if (e.key === "Escape") onClose();
              if (e.key === "ArrowDown") {
                e.preventDefault();
                setCursor((c) => Math.min(c + 1, hits.length - 1));
              }
              if (e.key === "ArrowUp") {
                e.preventDefault();
                setCursor((c) => Math.max(c - 1, 0));
              }
              if (e.key === "Enter") {
                e.preventDefault();
                run(hits[cursor]);
              }
            }}
          />
          <kbd className="rounded border border-line bg-inset px-1.5 py-0.5 font-mono text-[10px] text-dim">esc</kbd>
        </div>
        <div className="min-h-0 flex-1 overflow-y-auto p-2">
          {q.trim() === "" && (
            <p className="px-3 py-6 text-sm text-dim">
              {lang === "es"
                ? "Escribe para buscar en títulos, resúmenes y todo el contenido de los artículos."
                : "Type to search titles, summaries, and the full text of every article."}
            </p>
          )}
          {q.trim() !== "" && hits.length === 0 && (
            <p className="px-3 py-6 text-sm text-dim">
              {lang === "es" ? `Sin resultados para “${q.trim()}”.` : `No results for “${q.trim()}”.`}
            </p>
          )}
          {hits.map((hit, i) => (
            <button
              key={hit.article.slug}
              type="button"
              className={`block w-full rounded-xl px-3 py-2.5 text-left ${i === cursor ? "bg-accent-soft" : "hover:bg-inset"}`}
              onMouseEnter={() => setCursor(i)}
              onClick={() => run(hit)}
            >
              <p className="text-[10px] font-bold uppercase tracking-[0.8px] text-accent">
                {sectionOf(hit.article.slug)?.title[lang]}
              </p>
              <strong className={cx("block text-sm", i === cursor && "text-accent")}>{hit.article.title[lang]}</strong>
              {hit.excerpt && <span className="mt-0.5 block text-xs text-dim">{hit.excerpt}</span>}
            </button>
          ))}
        </div>
      </div>
    </div>
  );
}

export default function App() {
  const [lang, setLang] = useState<Lang>(readLang);
  const [theme, setTheme] = useState<Theme>(readTheme);
  const [query, setQuery] = useState("");
  const [slug, setSlug] = useState<string | null>(() => currentHashSlug());
  const [menuOpen, setMenuOpen] = useState(false);
  const [openParts, setOpenParts] = useState<Record<string, boolean>>({});
  const [searchOpen, setSearchOpen] = useState(false);

  useEffect(() => {
    localStorage.setItem(LANG_KEY, lang);
    document.documentElement.lang = lang;
  }, [lang]);

  useEffect(() => {
    document.documentElement.dataset.theme = theme;
    localStorage.setItem(THEME_KEY, theme);
  }, [theme]);

  useEffect(() => {
    const onHash = () => setSlug(currentHashSlug());
    window.addEventListener("hashchange", onHash);
    return () => window.removeEventListener("hashchange", onHash);
  }, []);

  const select = (next: string) => {
    setSlug(next);
    setMenuOpen(false);
    window.location.hash = `/${next}`;
    document.getElementById("doc-scroll")?.scrollTo({ top: 0 });
  };

  const needle = query.trim().toLowerCase();
  const matches = (text: string) => !needle || text.toLowerCase().includes(needle);
  const searchIn = (a: Article) =>
    matches(a.title[lang]) ||
    matches(a.summary[lang]) ||
    matches(a.slug) ||
    articleBody(lang, a.slug).toLowerCase().includes(needle);
  const grouped = sections
    .map((s) => ({ ...s, items: s.articles.filter(searchIn) }))
    .filter((s) => s.items.length > 0);

  const active = articles.find((a) => a.slug === slug) ?? null;
  const body = active ? articleBody(lang, active.slug) : "";

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "k") {
        e.preventDefault();
        setSearchOpen(true);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  const partOpen = (id: string) => needle.length > 0 || (openParts[id] ?? true);
  const togglePart = (id: string) =>
    setOpenParts((prev) => ({ ...prev, [id]: !partOpen(id) }));
  const partNumber = (id: string) => sections.findIndex((s) => s.id === id) + 1;

  return (
    <div className={cx(shell, menuOpen && "menu-open")}>
      <header className={topbar}>
        <button
          className={cx(iconBtn, menuToggle)}
          aria-label="menu"
          aria-expanded={menuOpen}
          onClick={() => setMenuOpen((o) => !o)}
        >
          ☰
        </button>
        <button
          className={brand}
          onClick={() => select(articles[0]?.slug ?? "")}
        >
          <span>
            Ira <em>docs</em>
          </span>
        </button>
        <div className="ml-auto flex items-center gap-[10px]">
          <button
            type="button"
            className={cx("hidden items-center gap-2 rounded-[10px] border border-line bg-inset px-3 py-[7px] text-sm text-dim hover:border-accent sm:flex")}
            onClick={() => setSearchOpen(true)}
          >
            <span aria-hidden>⌕</span>
            {lang === "es" ? "Buscar…" : "Search…"}
            <kbd className="rounded border border-line bg-raised px-1.5 font-mono text-[10px]">Ctrl K</kbd>
          </button>
          <button
            type="button"
            className={cx(iconBtn, "sm:hidden")}
            aria-label={lang === "es" ? "Buscar" : "Search"}
            onClick={() => setSearchOpen(true)}
          >
            ⌕
          </button>
          <div className={langToggle} role="group" aria-label="language">
            <button
              className={lang === "en" ? langOn : ""}
              onClick={() => setLang("en")}
              aria-pressed={lang === "en"}
            >
              EN
            </button>
            <button
              className={lang === "es" ? langOn : ""}
              onClick={() => setLang("es")}
              aria-pressed={lang === "es"}
            >
              ES
            </button>
          </div>
          <button
            className={iconBtn}
            aria-label="theme"
            onClick={() => setTheme((t) => (t === "dark" ? "light" : "dark"))}
          >
            {theme === "dark" ? "☀" : "☾"}
          </button>
        </div>
      </header>

      {menuOpen && (
        <button
          className={scrim}
          aria-label="close menu"
          onClick={() => setMenuOpen(false)}
        />
      )}

      {searchOpen && (
        <SearchModal
          lang={lang}
          onClose={() => setSearchOpen(false)}
          onSelect={(next) => {
            select(next);
            setSearchOpen(false);
          }}
        />
      )}

      <nav className={sidebar} aria-label="documentation">
        <input
          className={search}
          type="search"
          placeholder={lang === "es" ? "Buscar artículos…" : "Search articles…"}
          value={query}
          onChange={(e) => setQuery(e.target.value)}
        />
        <div className="flex-1 overflow-y-auto px-3 pb-6">
          {grouped.map((group) => (
            <section key={group.id}>
              <button
                type="button"
                className={cx(tocHead, partOpen(group.id) && tocHeadOpen)}
                aria-expanded={partOpen(group.id)}
                onClick={() => togglePart(group.id)}
              >
                <span className="col-span-full text-[10px] font-bold tracking-[0.8px] text-accent uppercase">{lang === "es" ? `Parte ${partNumber(group.id)}` : `Part ${partNumber(group.id)}`}</span>
                <h2 className="m-0 text-[13px] font-bold tracking-[0.2px]">{group.title[lang]}</h2>
                <span className={cx("text-xs text-dim transition-transform duration-[160ms]", partOpen(group.id) && "rotate-180")} aria-hidden>▾</span>
              </button>
              {partOpen(group.id) && (
                <div className="pt-1">
                  {group.items.map((a) => (
                    <button
                      key={a.slug}
                      className={cx(tocItem, active?.slug === a.slug && tocItemOn)}
                      onClick={() => select(a.slug)}
                    >
                      <strong className="mb-0.5 block text-sm font-semibold">{a.title[lang]}</strong>
                      <span className="block text-xs text-dim">
                        {(needle && excerpt(articleBody(lang, a.slug), needle)) || a.summary[lang]}
                      </span>
                    </button>
                  ))}
                </div>
              )}
            </section>
          ))}
          {grouped.length === 0 && (
            <p className="px-1 py-2 text-sm text-dim">
              {lang === "es" ? "Sin resultados." : "No results."}
            </p>
          )}
        </div>
      </nav>

      <main className="min-h-0 overflow-y-auto [grid-area:main]" id="doc-scroll">
        {active ? (
          <article className={doc}>
            <p className="mb-1.5 text-xs font-bold tracking-[0.8px] text-accent uppercase">{sectionOf(active.slug)?.title[lang]}</p>
            <h1 className="mb-5 text-[32px]">{active.title[lang]}</h1>
            <DocParts lang={lang} body={body} onSelect={select} />
            <DocNav
              lang={lang}
              articles={articles}
              slug={active.slug}
              onSelect={select}
            />
          </article>
        ) : (
          <article className={doc}>
            <h1 className="mb-5 text-[32px]">{lang === "es" ? "Documentación técnica de Ira" : "Ira technical documentation"}</h1>
            <p>
              {lang === "es"
                ? "Elige un artículo en el índice. La documentación cubre arquitectura, crates, API, voz, despliegue y guías de extensión."
                : "Pick an article from the index. The documentation covers architecture, crates, APIs, voice, deployment, and extension guides."}
            </p>
            <div className="mt-7 flex flex-col gap-7">
              {sections.map((section) => {
                const open = partOpen(section.id);
                return (
                  <section key={section.id} className="overflow-hidden rounded-[14px] border border-line bg-raised">
                    <button
                      type="button"
                      className={cx("grid w-full grid-cols-[auto_1fr_auto_auto] items-center gap-3 px-[18px] py-4 text-left hover:bg-inset", open && "border-b border-line")}
                      aria-expanded={open}
                      onClick={() => togglePart(section.id)}
                    >
                      <span className="text-[11px] font-bold tracking-[0.8px] text-accent uppercase">
                        {lang === "es" ? `Parte ${partNumber(section.id)}` : `Part ${partNumber(section.id)}`}
                      </span>
                      <h2 className="m-0 text-lg">{section.title[lang]}</h2>
                      <span className="rounded-full border border-line bg-inset px-[9px] py-0.5 text-xs font-bold text-dim">{section.articles.length}</span>
                      <span className={cx("text-[13px] text-dim transition-transform duration-[160ms]", open && "rotate-180")} aria-hidden>▾</span>
                    </button>
                    {open && (
                      <div className="grid grid-cols-[repeat(auto-fill,minmax(250px,1fr))] gap-3 px-[18px] py-4">
                        {section.articles.map((a) => (
                          <button key={a.slug} className={card} onClick={() => select(a.slug)}>
                            <strong>{a.title[lang]}</strong>
                            <span>{a.summary[lang]}</span>
                          </button>
                        ))}
                      </div>
                    )}
                  </section>
                );
              })}
            </div>
          </article>
        )}
      </main>
    </div>
  );
}

function DocParts({
  lang,
  body,
  onSelect,
}: {
  lang: Lang;
  body: string;
  onSelect: (slug: string) => void;
}) {
  const { intro, parts } = useMemo(() => splitParts(body), [body]);
  const [open, setOpen] = useState<Record<number, boolean>>({});
  useEffect(() => setOpen({}), [body]);
  const allOpen = parts.length > 0 && parts.every((_, i) => open[i] !== false);
  const introText = intro.replace(/^# .*\n+/, "");
  return (
    <>
      {introText.trim() && (
        <Md text={introText} onSelect={onSelect} />
      )}
      <div className="mt-2 mb-3 flex justify-end">
        <button
          type="button"
          className="text-sm font-semibold text-accent"
          onClick={() =>
            setOpen(allOpen
              ? Object.fromEntries(parts.map((_, i) => [i, false]))
              : Object.fromEntries(parts.map((_, i) => [i, true])))
          }
        >
          {allOpen
            ? lang === "es"
              ? "Cerrar todo"
              : "Collapse all"
            : lang === "es"
              ? "Abrir todo"
              : "Expand all"}
        </button>
      </div>
      {parts.map((part, i) => {
        const partOpen = open[i] !== false;
        return (
          <section key={part.title} className="mt-3 overflow-hidden rounded-[14px] border border-line bg-raised">
            <button
              type="button"
              className={cx("grid w-full grid-cols-[auto_1fr_auto] items-center gap-3 px-[18px] py-4 text-left hover:bg-inset", partOpen && "border-b border-line")}
              aria-expanded={partOpen}
              onClick={() => setOpen((p) => ({ ...p, [i]: !partOpen }))}
            >
              <span className="text-[11px] font-bold tracking-[0.8px] text-accent">{String(i + 1).padStart(2, "0")}</span>
              <h2 className="m-0 text-lg">{part.title}</h2>
              <span className={cx("text-[13px] text-dim transition-transform duration-[160ms]", partOpen && "rotate-180")} aria-hidden>▾</span>
            </button>
            {partOpen && <div className="px-[18px] pb-4"><Md text={part.body} onSelect={onSelect} /></div>}
          </section>
        );
      })}
    </>
  );
}

function Md({
  text,
  onSelect,
}: {
  text: string;
  onSelect: (slug: string) => void;
}) {
  return (
    <div className={md.root}>
      <ReactMarkdown
        remarkPlugins={[remarkGfm]}
        urlTransform={defaultUrlTransform}
        components={{
          h1: (props: ComponentProps<"h1">) => <h1 className={md.h1} {...props} />,
          h2: (props: ComponentProps<"h2">) => <h2 className={md.h2} {...props} />,
          h3: (props: ComponentProps<"h3">) => <h3 className={md.h3} {...props} />,
          h4: (props: ComponentProps<"h4">) => <h4 className={md.h4} {...props} />,
          p: (props: ComponentProps<"p">) => <p className={md.p} {...props} />,
          ul: (props: ComponentProps<"ul">) => <ul className={md.list} {...props} />,
          ol: (props: ComponentProps<"ol">) => <ol className={md.list} {...props} />,
          blockquote: (props: ComponentProps<"blockquote">) => <blockquote className={md.quote} {...props} />,
          hr: () => <hr className={md.hr} />,
          pre: (props: ComponentProps<"pre">) => <pre className={md.pre} {...props} />,
          code: ({ className, ...props }: ComponentProps<"code">) => <code className={className ?? md.code} {...props} />,
          th: (props: ComponentProps<"th">) => <th className={`${md.cell} ${md.th}`} {...props} />,
          td: (props: ComponentProps<"td">) => <td className={md.cell} {...props} />,
          a: ({ href, children }) => {
            const external = href?.startsWith("http");
                    return external ? (
                      <a className={md.a} href={href} target="_blank" rel="noreferrer noopener">
                {children}
              </a>
            ) : (
                      <a
                        className={md.a}
                        href={href}
                onClick={(e) => {
                  if (href?.startsWith("#/")) {
                    e.preventDefault();
                    onSelect(decodeURIComponent(href.slice(2)));
                  }
                }}
              >
                {children}
              </a>
            );
          },
          table: ({ children }) => (
                    <div className={md.wrap}>
                      <table className={md.table}>{children}</table>
            </div>
          ),
        }}
      >
        {text}
      </ReactMarkdown>
    </div>
  );
}

function DocNav({
  lang,
  articles,
  slug,
  onSelect,
}: {
  lang: Lang;
  articles: Article[];
  slug: string;
  onSelect: (slug: string) => void;
}) {
  const i = articles.findIndex((a) => a.slug === slug);
  const prev = i > 0 ? articles[i - 1] : null;
  const next = i >= 0 && i < articles.length - 1 ? articles[i + 1] : null;
  if (!prev && !next) return null;
  return (
    <nav className="mt-12 flex justify-between gap-3 border-t border-line pt-5 [&_button]:max-w-[48%] [&_button]:rounded-xl [&_button]:border [&_button]:border-line [&_button]:bg-raised [&_button]:px-[14px] [&_button]:py-[10px] [&_button]:text-sm hover:[&_button]:border-accent">
      {prev ? (
        <button onClick={() => onSelect(prev.slug)}>
          <span>← {prev.title[lang]}</span>
        </button>
      ) : (
        <span />
      )}
      {next ? (
        <button className="text-right" onClick={() => onSelect(next.slug)}>
          <span>{next.title[lang]} →</span>
        </button>
      ) : (
        <span />
      )}
    </nav>
  );
}
