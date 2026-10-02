import { cx } from "../ui";

export function Activity({ receiving = false }: { receiving?: boolean }) {
  return <span className={cx("activity-signal", receiving && "activity-receiving")} aria-hidden="true">
    <svg viewBox="0 0 20 20" fill="none"><circle cx="10" cy="10" r="7" stroke="currentColor" opacity=".2" /><g className="activity-orbit"><path d="M10 3a7 7 0 0 1 7 7" stroke="currentColor" strokeWidth="2" strokeLinecap="round" /><circle cx="10" cy="10" r="2" fill="currentColor" /></g></svg>
  </span>;
}
