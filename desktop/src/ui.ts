export function cx(...parts: Array<string | false | null | undefined>) {
  return parts.filter((part): part is string => typeof part === "string" && part.length > 0).join(" ");
}

const press = "inline-flex items-center justify-center gap-[0.4rem] font-semibold leading-none transition-[background,color,border-color,opacity] duration-150 [&_svg]:size-4 in-catalog:min-h-11 in-catalog:leading-normal max-[600px]:min-h-11";

export const btn = {
  primary: `${press} rounded-[10px] border-0 bg-primary px-[0.85rem] py-2 text-[0.9rem] text-on-primary hover:brightness-[1.08]`,
  secondary: `${press} rounded-[10px] border border-line bg-elevated px-[0.85rem] py-2 text-[0.9rem] text-ink hover:border-muted hover:bg-surface`,
  danger: `${press} rounded-[10px] border border-danger bg-transparent px-[0.85rem] py-2 text-[0.9rem] text-danger hover:bg-danger hover:text-white`,
  ghost: `${press} rounded-[10px] border-0 bg-transparent px-[0.85rem] py-2 text-[0.9rem] text-muted hover:bg-elevated hover:text-ink`,
  sm: "!px-[0.6rem] !py-[0.32rem] !text-[0.8rem]",
  icon: "inline-flex size-11 shrink-0 items-center justify-center rounded-[10px] border-0 bg-transparent p-0 text-muted hover:bg-elevated hover:text-ink [&_svg]:size-5",
  send: "inline-flex size-10 shrink-0 items-center justify-center rounded-xl border-0 bg-primary p-0 text-on-primary transition-[background,color,opacity] duration-150 hover:brightness-[1.08] [&_svg]:size-5 [&_svg]:transition-transform [&_svg]:duration-[180ms] [&_svg]:ease-[cubic-bezier(0.16,1,0.3,1)] hover:[&_svg]:-translate-y-0.5 phone:size-11",
  mic: "inline-flex size-10 shrink-0 items-center justify-center rounded-xl border border-line bg-elevated p-0 text-ink transition-[background,color,border-color,opacity] duration-150 hover:border-muted [&_svg]:size-5 phone:size-11",
  micOn: "border-listen bg-listen text-[#0c0c0d]",
};

export const scrim = "fixed inset-0 z-[5] border-0 bg-black/45";
export const scrimSettings = "fixed inset-0 z-[6] border-0 bg-black/45";
export const scrimVoice = "fixed inset-0 z-[8] border-0 bg-black/45";

export const sheet = "sheet catalog fixed inset-y-0 right-0 z-[7] w-[min(850px,100%)] overflow-auto border-l border-line bg-surface px-8 pt-[1.1rem] pb-8 shadow-[-18px_0_70px_rgba(9,4,18,0.22)] outline-none [scrollbar-width:thin] [overscroll-behavior:contain] mobile:pt-[max(1.1rem,env(safe-area-inset-top))] mobile:pb-[max(2rem,env(safe-area-inset-bottom))] phone:px-4";
export const sheetHead = "sticky top-0 z-[1] mb-6 flex items-start justify-between gap-4 border-b border-line bg-surface py-6 phone:py-4 [&_h2]:m-0 [&_h2]:text-[1.6rem] [&_h2]:leading-[1.2] [&_h2]:font-[650] [&_h2]:tracking-[-0.03em] phone:[&_h2]:text-[1.4rem] [&_p]:mt-[0.4rem] [&_p]:text-[0.9rem] [&_p]:text-muted [&_button]:shrink-0";
export const sheetErr = "text-[0.85rem] text-danger wrap-anywhere rounded-lg border border-danger px-4 py-3";

export const dot = "size-2 shrink-0 rounded-full bg-line";
export const dotOn = "bg-listen";

export const navItem = "flex min-h-11 w-full items-center gap-[0.6rem] rounded-[10px] border-0 bg-transparent px-[0.7rem] py-[0.55rem] text-left font-[550] text-ink hover:bg-elevated [&_svg]:size-[18px] collapsed:!justify-center collapsed:!px-0";
export const labelHide = "min-w-0 truncate collapsed:!hidden";

export const sheetDocs = "sheet catalog docs-sheet fixed inset-y-0 right-0 z-[7] flex w-[min(850px,100%)] flex-col overflow-hidden border-l border-line bg-surface p-0 shadow-[-18px_0_70px_rgba(9,4,18,0.22)] outline-none mobile:pt-[env(safe-area-inset-top)] mobile:pb-[env(safe-area-inset-bottom)]";
export const docsHead = "m-0 flex shrink-0 items-start justify-between gap-4 border-b border-line bg-surface px-8 py-[1.1rem] phone:px-4 [&_h2]:m-0 [&_h2]:text-[1.6rem] phone:[&_h2]:text-[1.4rem] [&_h2]:leading-[1.2] [&_h2]:font-[650] [&_h2]:tracking-[-0.03em] [&_p]:mt-[0.4rem] [&_p]:text-[0.9rem] [&_p]:text-muted";
export const docsFrame = "min-h-0 w-full flex-1 border-0 bg-bg [color-scheme:light_dark]";

export const catalogCurrent = "flex items-center gap-3 rounded-xl bg-[color-mix(in_srgb,var(--color-brand)_12%,var(--color-bg))] p-4 phone:flex-wrap [&_strong]:block [&_strong]:font-semibold [&_strong]:wrap-anywhere";
export const catalogLayout = "mt-8 grid min-w-0 grid-cols-[11rem_minmax(0,1fr)] gap-6 phone:mt-6 phone:grid-cols-1 phone:gap-6";
export const catalogLayoutEmpty = "mt-0 grid-cols-1";
export const catalogNav = "empty:hidden";
export const catalogProviders = "mt-3 grid list-none gap-1 p-0 phone:flex phone:overflow-x-auto phone:pb-2 phone:[&>li]:shrink-0 phone:[&>li]:basis-40";
export const catalogProvider = "grid w-full gap-1 rounded-[10px] border border-transparent bg-transparent p-3 text-left wrap-anywhere hover:bg-bg";
export const catalogProviderOn = "border-transparent bg-[color-mix(in_srgb,var(--color-accent)_10%,var(--color-surface))]";
export const catalogDetail = "min-w-0 border-l border-line pl-6 empty:border-0 empty:p-0 phone:border-t phone:border-l-0 phone:pt-5 phone:pl-0";
export const catalogEmpty = "py-6 text-[0.9rem] text-muted [&_p]:mb-3 [&_p]:mt-0";
export const sectionHead = "mb-3 flex flex-wrap items-center justify-between gap-2 [&_h4]:m-0 [&_h4]:text-[0.9rem] [&_h4]:font-semibold [&_span]:text-[0.8rem] [&_span]:font-[450] [&_span]:text-muted [&_span]:tabular-nums";
export const modelPick = "flex min-h-14 min-w-0 flex-1 items-center gap-3 rounded-lg border-0 bg-transparent p-3 text-left hover:bg-bg disabled:opacity-100";
export const confirm = "flex min-w-0 flex-wrap items-center gap-[0.4rem] p-3 text-[0.85rem] wrap-anywhere [&_span]:w-full";
export const settings = "mt-6 border-t border-line [&_summary]:cursor-pointer [&_summary]:py-4 [&_summary]:text-[0.9rem] [&_summary]:font-semibold hover:[&_summary]:text-accent [&_summary_span]:mt-[0.2rem] [&_summary_span]:block [&_summary_span]:text-[0.8rem] [&_summary_span]:font-[450] [&_summary_span]:text-muted";
export const settingsBody = "py-2 pb-4";

export const field = "mb-4 grid min-w-0 gap-[0.45rem] [&_label]:text-[0.84rem] [&_label]:font-semibold [&_label]:text-ink";
export const fieldInvalid = "[&_small]:!text-danger";
export const control = "group relative min-w-0 rounded-[11px] border border-line bg-bg transition-[border-color,background] duration-[160ms] focus-within:border-accent focus-within:bg-surface has-[:focus-visible]:outline has-[:focus-visible]:outline-2 has-[:focus-visible]:outline-offset-[3px] has-[:focus-visible]:outline-accent has-[:disabled]:opacity-60";
export const controlInvalid = "!border-danger";
export const controlInput = "block w-full min-w-0 min-h-[46px] rounded-[inherit] border-0 bg-transparent px-[0.85rem] py-[0.7rem] text-[0.94rem] text-ink outline-none placeholder:text-muted placeholder:opacity-100";
export const controlArea = "min-h-[110px] resize-y";
export const focusTrack = "pointer-events-none absolute right-3 -bottom-px left-3 h-0.5 origin-center scale-x-0 bg-accent transition-transform duration-[250ms] ease-[cubic-bezier(0.16,1,0.3,1)] group-focus-within:scale-x-100";
export const fieldIcon = "pointer-events-none absolute top-[13px] left-[0.85rem] flex text-muted [&_svg]:size-[19px]";
export const fieldAction = "absolute top-[3px] right-[3px] grid size-10 place-items-center rounded-lg border-0 bg-transparent p-0 text-muted hover:bg-elevated hover:text-accent";
export const fieldHint = "text-[0.78rem] leading-[1.45] text-muted";

export const turnUser = "turn user mx-auto mb-8 flex w-full max-w-[50rem] flex-col items-end px-6 phone:px-4";
export const turnIra = "turn ira mx-auto mb-8 grid w-full max-w-[50rem] grid-cols-[34px_minmax(0,1fr)] items-start gap-x-3 px-6 phone:px-4";
export const turnError = "turn error mx-auto mb-8 w-full max-w-[50rem] px-6 phone:px-4";
export const whoIra = "m-0 flex items-center [&_img]:size-[34px] [&_img]:object-contain";
export const whoUser = "mb-[0.35rem] flex items-center pr-1 text-xs font-[650] text-muted";
export const whoError = "mb-[0.65rem] flex items-center gap-2 text-[0.85rem] font-[650] text-muted";
export const bubbleUser = "max-w-[min(36rem,85%)] rounded-[16px_16px_4px_16px] bg-bubble px-[1.1rem] py-[0.9rem] font-[450] text-white phone:max-w-[90%] [&_p]:m-0 [&_p]:wrap-anywhere [&_p]:whitespace-pre-wrap";
export const bubbleIra = "min-w-0 pl-0 leading-[1.7] text-ink";
export const bubbleError = "rounded-xl border border-[color-mix(in_srgb,var(--color-danger)_45%,var(--color-line))] bg-[color-mix(in_srgb,var(--color-danger)_5%,var(--color-surface))] p-4 text-danger [&_p]:m-0 [&_p]:wrap-anywhere [&_p]:whitespace-pre-wrap";
export const bubbleLoad = "flex min-w-0 items-center gap-2 pl-0 leading-[1.7] text-muted";

export const composer = "group relative flex flex-col gap-[0.85rem] rounded-2xl border border-line bg-surface p-4 pb-[0.8rem] transition-colors duration-[180ms] focus-within:border-accent after:pointer-events-none after:absolute after:-bottom-px after:left-[20%] after:right-[20%] after:h-0.5 after:origin-center after:scale-x-0 after:bg-accent after:transition-transform after:duration-[280ms] after:ease-[cubic-bezier(0.16,1,0.3,1)] focus-within:after:scale-x-100 phone:gap-2 phone:p-[0.8rem]";
export const composerBox = "w-full resize-none border-0 bg-transparent px-1 py-[0.15rem] text-[1.06rem] outline-none placeholder:text-muted";
export const composerBar = "flex min-w-0 items-center gap-[0.4rem] phone:flex-wrap phone:gap-1";
export const modelSelect = "max-w-[min(15rem,35%)] min-h-[34px] truncate rounded-lg border-0 bg-transparent p-[0.4rem] text-[0.82rem] font-semibold phone:!min-h-11 phone:!max-w-none phone:!min-w-0 phone:!flex-1 phone:!text-[0.85rem]";
export const effortSelect = "max-w-[6.5rem] min-h-[34px] shrink-0 truncate rounded-lg border-0 bg-transparent p-[0.4rem] text-[0.82rem] font-semibold phone:min-h-11";
export const effortCatalog = "mr-[0.35rem] min-h-8 shrink-0 rounded-full border border-line bg-elevated px-[0.4rem] text-[0.82rem] font-semibold";
export const chip = "inline-flex min-h-[34px] items-center justify-center gap-[0.35rem] rounded-lg border-0 px-[0.55rem] py-[0.4rem] text-[0.82rem] font-semibold text-muted hover:text-ink [&_svg]:size-[17px] phone:min-h-11 phone:px-[0.65rem] chip:[&_span]:hidden";
export const chipOn = "bg-[color-mix(in_srgb,var(--color-accent)_12%,var(--color-surface))] text-accent";

export const activity = "inline-flex h-5 w-[27px] shrink-0 items-center gap-[3px] text-accent in-primary:text-current";
export const bar = "block h-3 w-1 rounded-[3px] bg-current animate-signal";

export const voiceCard = "fixed top-1/2 left-1/2 z-[9] w-[min(22rem,calc(100vw-2rem))] [translate:-50%_-50%] rounded-2xl bg-surface px-6 pt-8 pb-6 text-center shadow-float";
export const voiceHalo = "relative mx-auto mt-[0.55rem] mb-4 size-[9.5rem] before:absolute before:inset-0 before:animate-voice-ring before:rounded-[40%_60%_55%_45%] before:border before:border-accent before:content-[''] after:absolute after:inset-0 after:animate-voice-ring after:rounded-[40%_60%_55%_45%] after:border after:border-accent after:[animation-delay:1s] after:content-[''] data-[phase=wait]:before:[animation-duration:1.4s] data-[phase=wait]:after:[animation-duration:1.4s] data-[phase=connect]:before:[animation-duration:1.4s] data-[phase=connect]:after:[animation-duration:1.4s] data-[phase=error]:before:[animation-duration:1.4s] data-[phase=error]:after:[animation-duration:1.4s]";
export const voiceOrb = "absolute inset-[1.15rem] rounded-[40%_60%_55%_45%] bg-[color-mix(in_srgb,var(--color-brand)_18%,var(--color-surface))] transition-colors duration-200 [transform:scale(calc(1+var(--voice-level)*0.48))] data-[phase=speak]:bg-[color-mix(in_srgb,var(--color-accent)_28%,var(--color-surface))] data-[phase=error]:bg-danger data-[phase=wait]:animate-voice-wait data-[phase=connect]:animate-voice-wait [&_img]:size-full [&_img]:object-contain";

export const md = {
  root: "wrap-anywhere [&>:first-child]:mt-0 [&>:last-child]:mb-0",
  p: "mb-[0.75em] last:mb-0",
  h: "mt-[1.2em] mb-[0.35em] text-[1.15em] leading-[1.35] font-[650] tracking-normal first:mt-0 [&_strong]:font-[inherit]",
  hSmall: "mt-[1.2em] mb-[0.35em] text-[1em] leading-[1.35] font-[650] tracking-normal [&_strong]:font-[inherit]",
  ul: "mb-[0.75em] list-disc pl-[1.35em] last:mb-0 [&_li+li]:mt-[0.2em] [&_li>ul]:mt-[0.25em] [&_li>ul]:mb-0 [&_li>ol]:mt-[0.25em] [&_li>ol]:mb-0",
  ol: "mb-[0.75em] list-decimal pl-[1.35em] last:mb-0 [&_li+li]:mt-[0.2em] [&_li>ul]:mt-[0.25em] [&_li>ul]:mb-0 [&_li>ol]:mt-[0.25em] [&_li>ol]:mb-0",
  a: "text-accent underline-offset-[0.18em]",
  code: "rounded-md bg-elevated px-[0.38em] py-[0.12em] font-mono text-[0.88em]",
  pre: "mb-[0.75em] overflow-x-auto rounded-xl bg-sidebar p-4 last:mb-0 [scrollbar-width:thin] [&_code]:bg-transparent [&_code]:p-0 [&_code]:font-mono [&_code]:text-[0.82rem] [&_code]:leading-[1.45]",
  quote: "mb-[0.75em] ml-0 border-l border-line pl-[0.85em] text-muted last:mb-0",
  hr: "my-[0.9em] border-0 border-t border-line",
  img: "h-auto max-w-full rounded-[10px]",
  tableWrap: "mb-[0.75em] overflow-x-auto last:mb-0 [scrollbar-width:thin]",
  table: "w-full border-collapse text-[0.92rem]",
  cell: "border border-line px-[0.65rem] py-[0.4rem] text-left",
  th: "bg-elevated font-semibold",
  check: "mr-[0.4em]",
};
