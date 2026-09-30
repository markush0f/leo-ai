import { activity, bar, cx } from "../ui";

export function Activity({ receiving = false }: { receiving?: boolean }) {
  return <span className={cx(activity, receiving && "[&_i]:[animation-duration:650ms]")} aria-hidden="true">
    <i className={bar} /><i className={cx(bar, "[animation-delay:-900ms]")} /><i className={cx(bar, "[animation-delay:-600ms]")} /><i className={cx(bar, "[animation-delay:-300ms]")} />
  </span>;
}
