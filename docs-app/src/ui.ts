export function cx(...parts: Array<string | false | null | undefined>) {
  return parts.filter((part): part is string => typeof part === "string" && part.length > 0).join(" ");
}

export const shell = "grid h-full grid-cols-[300px_1fr] grid-rows-[56px_1fr] [grid-template-areas:'top_top'_'side_main'] narrow:grid-cols-1 narrow:[grid-template-areas:'top'_'main']";
export const topbar = "flex items-center gap-3 border-b border-line bg-raised px-4 [grid-area:top]";
export const iconBtn = "grid size-9 place-items-center rounded-[10px] border border-line bg-inset text-base leading-none hover:border-accent";
export const menuToggle = "hidden narrow:block";
export const brand = "flex items-center text-lg font-bold tracking-[0.3px] [&_em]:font-normal [&_em]:text-accent [&_em]:not-italic";
export const lang = "flex overflow-hidden rounded-[10px] border border-line [&_button]:px-3 [&_button]:py-2 [&_button]:text-[13px] [&_button]:font-semibold [&_button]:text-dim";
export const langOn = "bg-accent text-white";
export const sidebar = "flex min-h-0 flex-col border-r border-line bg-raised [grid-area:side] narrow:fixed narrow:top-14 narrow:bottom-0 narrow:left-0 narrow:z-30 narrow:w-[min(320px,86vw)] narrow:-translate-x-[105%] narrow:shadow-[12px_0_32px_rgba(0,0,0,0.35)] narrow:transition-transform narrow:duration-[180ms] menu-open:translate-x-0";
export const search = "mx-3 mt-3 rounded-[10px] border border-line bg-inset px-3 py-[9px] text-ink outline-none focus:border-accent focus:shadow-[0_0_0_3px_var(--color-accent-soft)]";
export const tocHead = "mt-[14px] grid w-full grid-cols-[1fr_auto] items-center gap-x-2 gap-y-1 rounded-[10px] border border-transparent px-2 py-[10px] text-left hover:bg-inset";
export const tocHeadOpen = "border-line bg-inset";
export const tocItem = "block w-full rounded-[10px] px-[10px] py-[9px] text-left hover:bg-inset";
export const tocItemOn = "bg-accent-soft [&_strong]:text-accent";
export const doc = "mx-auto max-w-[860px] px-8 pt-10 pb-24 narrow:px-4 narrow:pt-6 narrow:pb-[72px]";
export const card = "rounded-[14px] border border-line bg-raised p-4 text-left hover:border-accent [&_strong]:mb-1 [&_strong]:block [&_strong]:text-[15px] [&_span]:text-[13px] [&_span]:text-dim";
export const scrim = "fixed inset-x-0 top-14 bottom-0 z-20 hidden border-0 bg-black/45 p-0 narrow:block";

export const md = {
  root: "text-[15.5px] leading-[1.65] break-words",
  h1: "mt-[2em] mb-[0.5em] text-[26px] leading-[1.25]",
  h2: "mt-[2em] mb-[0.5em] border-b border-line pb-1.5 text-[21px] leading-[1.25]",
  h3: "mt-[2em] mb-[0.5em] text-[17px] leading-[1.25]",
  h4: "mt-[2em] mb-[0.5em] leading-[1.25]",
  p: "my-[0.8em]",
  list: "my-[0.8em] [&_li>ul]:my-[0.2em] [&_li>ol]:my-[0.2em]",
  a: "text-accent no-underline hover:underline",
  code: "rounded-[5px] border border-line bg-inset px-[5px] py-px font-mono text-[0.88em]",
  pre: "overflow-x-auto rounded-xl border border-line bg-code px-4 py-[14px] text-[#e8eaee] [&_code]:border-0 [&_code]:bg-transparent [&_code]:p-0 [&_code]:font-mono [&_code]:text-[13px] [&_code]:leading-[1.55]",
  quote: "my-4 rounded-r-[10px] border-l-[3px] border-accent bg-inset px-4 py-0.5 text-dim",
  wrap: "my-4 overflow-x-auto",
  table: "w-full border-collapse text-sm",
  cell: "border border-line px-[11px] py-[7px] text-left align-top",
  th: "bg-inset font-bold",
  hr: "my-8 border-0 border-t border-line",
};
