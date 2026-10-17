import { connectionSnapshot } from "./query/connection";

// One explicit create entrance can activate its newly created composer. Merely
// selecting another historical thread never creates collaboration intent.
let pending: { id: string; revision: number; until: number } | undefined;
export function requestDesktopOpenIntent(id: string) {
  pending = { id, revision: connectionSnapshot().revision, until: Date.now() + 20_000 };
}
export function consumeDesktopOpenIntent(id: string): boolean {
  if (!pending) return false;
  const valid = pending.id === id && pending.revision === connectionSnapshot().revision && Date.now() <= pending.until;
  if (valid || Date.now() > pending.until || pending.revision !== connectionSnapshot().revision) pending = undefined;
  return valid;
}
