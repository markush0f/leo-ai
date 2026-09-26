type WaMessage = {
  conversation?: string | null;
  extendedTextMessage?: { text?: string | null } | null;
};

export function messageText(message: WaMessage | null | undefined): string | null {
  const text = message?.conversation ?? message?.extendedTextMessage?.text ?? "";
  const trimmed = text.trim();
  return trimmed ? trimmed : null;
}
