export function phoneDigits(jid: string): string {
  const user = jid.split("@")[0]?.split(":")[0] ?? "";
  return user.replace(/\D/g, "");
}

export function parseAllowPhones(raw: string | undefined): string[] {
  if (!raw) return [];
  return raw
    .split(",")
    .map((part) => part.replace(/\D/g, ""))
    .filter((digits) => digits.length > 0);
}

export function isPrivateDm(jid: string): boolean {
  return jid.endsWith("@s.whatsapp.net") || jid.endsWith("@lid");
}

export function isSelfChat(remoteJid: string, ownJids: string[]): boolean {
  const remote = phoneDigits(remoteJid);
  if (!remote) return false;
  return ownJids.some((own) => phoneDigits(own) === remote);
}

export function mayReply(input: {
  remoteJid: string;
  fromMe: boolean;
  ownJids: string[];
  allowPhones: string[];
}): boolean {
  if (!isPrivateDm(input.remoteJid)) return false;
  if (isSelfChat(input.remoteJid, input.ownJids)) return true;
  if (input.fromMe) return false;
  const phone = phoneDigits(input.remoteJid);
  return phone.length > 0 && input.allowPhones.includes(phone);
}
