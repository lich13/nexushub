export type SessionSelectionState = {
  scope: string;
  selecting: boolean;
  keys: string[];
  anchor: string | null;
  notice: string | null;
};

export type SessionClick = {
  detail: number;
  metaKey: boolean;
  shiftKey: boolean;
  ctrlKey: boolean;
};

export function isMacLikePlatform(): boolean {
  if (typeof navigator === "undefined") return true;
  const platform = navigator.platform || navigator.userAgent || "";
  return !/^Win/i.test(platform);
}

export function isToggleSelectionClick(click: SessionClick): boolean {
  if (click.detail === 0) return false;
  if (isMacLikePlatform()) {
    return click.metaKey && !click.ctrlKey;
  }
  return click.metaKey || click.ctrlKey;
}

export function emptySessionSelection(scope: string): SessionSelectionState {
  return { scope, selecting: false, keys: [], anchor: null, notice: null };
}

export function syncSessionSelection(
  state: SessionSelectionState,
  scope: string,
  visible: string[],
): SessionSelectionState {
  if (scope !== state.scope) return emptySessionSelection(scope);
  const keys = state.keys.filter(key => visible.includes(key));
  const selecting = keys.length > 0 && state.selecting;
  const anchor = state.anchor && visible.includes(state.anchor) ? state.anchor : null;
  return keys.length === state.keys.length &&
    selecting === state.selecting &&
    anchor === state.anchor
    ? state
    : { ...state, selecting, keys, anchor };
}

export function addSessionSelection(
  state: SessionSelectionState,
  keys: string[],
): SessionSelectionState {
  const combined = [...new Set([...state.keys, ...keys])];
  return { ...state, selecting: combined.length > 0, keys: combined, notice: null };
}

export function clickSessionSelection(
  state: SessionSelectionState,
  visible: string[],
  key: string,
  click: SessionClick,
  activeKey?: string | null,
): SessionSelectionState {
  if (isMacLikePlatform() && click.ctrlKey && click.detail !== 0) return state;
  const shift = click.detail !== 0 && click.shiftKey;
  const toggle = isToggleSelectionClick(click);

  if (shift) {
    const resolvedAnchor =
      state.anchor && visible.includes(state.anchor)
        ? state.anchor
        : activeKey && visible.includes(activeKey)
          ? activeKey
          : null;
    const start = resolvedAnchor ? visible.indexOf(resolvedAnchor) : -1;
    const end = visible.indexOf(key);
    const range =
      start >= 0 && end >= 0
        ? visible.slice(Math.min(start, end), Math.max(start, end) + 1)
        : [key];
    return {
      ...addSessionSelection(state, range),
      anchor: start >= 0 ? resolvedAnchor : key,
    };
  }

  if (toggle) {
    if (state.keys.includes(key)) {
      const nextKeys = state.keys.filter(value => value !== key);
      return {
        ...state,
        selecting: nextKeys.length > 0,
        keys: nextKeys,
        anchor: key,
        notice: null,
      };
    }
    const seed =
      !state.selecting &&
      state.keys.length === 0 &&
      activeKey &&
      activeKey !== key &&
      visible.includes(activeKey)
        ? [activeKey, key]
        : [key];
    return { ...addSessionSelection(state, seed), anchor: key };
  }

  return {
    ...state,
    selecting: false,
    keys: [],
    anchor: key,
    notice: null,
  };
}

type MessageTimeSummary = { id?: string; sessionKey?: string; updatedAt?: string | null };
export function compareSessionMessageTime(a: MessageTimeSummary, b: MessageTimeSummary): number {
  const time = (value?: string | null) =>
    value && Number.isFinite(Date.parse(value)) ? Date.parse(value) : -Infinity;
  const first = time(a.updatedAt);
  const second = time(b.updatedAt);
  return first === second ? 0 : first > second ? -1 : 1;
}
