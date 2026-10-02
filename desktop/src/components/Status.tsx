import type { ReactNode } from "react";
import { Activity } from "./Activity";
import { cx } from "../ui";

export function Status({ children, tone = "neutral", busy = false }: {
  children: ReactNode; tone?: "neutral" | "success" | "error"; busy?: boolean;
}) {
  return <span className={cx("resource-status", `status-${tone}`)}>
    {busy ? <Activity /> : <span className="status-dot" aria-hidden="true" />}{children}
  </span>;
}
