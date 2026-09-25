export function Activity({ receiving = false }: { receiving?: boolean }) {
  return <span className={`activity${receiving ? " receiving" : ""}`} aria-hidden="true">
    <i /><i /><i /><i />
  </span>;
}
