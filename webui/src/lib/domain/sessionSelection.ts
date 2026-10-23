export type SessionSelectionState = { scope: string; selecting: boolean; keys: string[]; anchor: string | null; notice: string | null };
export type SessionClick = { detail: number; metaKey: boolean; shiftKey: boolean; ctrlKey: boolean };

export function emptySessionSelection(scope: string): SessionSelectionState {
  return { scope, selecting: false, keys: [], anchor: null, notice: null };
}

export function syncSessionSelection(state: SessionSelectionState, scope: string, visible: string[]): SessionSelectionState {
  if (scope !== state.scope) return emptySessionSelection(scope);
  const keys = state.keys.filter(key => visible.includes(key));
  const anchor = state.anchor && visible.includes(state.anchor) ? state.anchor : null;
  return keys.length === state.keys.length && anchor === state.anchor ? state : { ...state, keys, anchor };
}

export function addSessionSelection(state: SessionSelectionState, keys: string[]): SessionSelectionState {
  const combined = [...new Set([...state.keys, ...keys])];
  return { ...state, selecting: true, keys: combined, notice: null };
}

export function clickSessionSelection(state: SessionSelectionState, visible: string[], key: string, click: SessionClick): SessionSelectionState {
  if (click.ctrlKey && click.detail !== 0) return state;
  const shift = click.detail !== 0 && click.shiftKey;
  const meta = click.detail !== 0 && click.metaKey;
  if (shift) {
    const start = state.anchor ? visible.indexOf(state.anchor) : -1;
    const end = visible.indexOf(key);
    const range = start >= 0 && end >= 0 ? visible.slice(Math.min(start, end), Math.max(start, end) + 1) : [key];
    return { ...addSessionSelection(state, range), anchor: start >= 0 ? state.anchor : key };
  }
  if (state.selecting || meta) {
    const next = state.keys.includes(key) ? { ...state, keys: state.keys.filter(value => value !== key), notice: null } : addSessionSelection(state, [key]);
    return { ...next, selecting: true, anchor: key };
  }
  return { ...state, anchor: key, notice: null };
}

type MessageTimeSummary = { id?: string; sessionKey?: string; updatedAt?: string | null };
export function compareSessionMessageTime(a: MessageTimeSummary, b: MessageTimeSummary): number {
  const time = (value?: string | null) => value && Number.isFinite(Date.parse(value)) ? Date.parse(value) : -Infinity;
  const first = time(a.updatedAt), second = time(b.updatedAt);
  return first === second
    ? String(a.sessionKey ?? a.id ?? "").localeCompare(String(b.sessionKey ?? b.id ?? ""))
    : first > second ? -1 : 1;
}
