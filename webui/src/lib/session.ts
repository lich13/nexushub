import type { SessionUser } from "../types";

const KEY = "nexushub-session";

export function saveSession(user: SessionUser) {
  window.localStorage.setItem(KEY, JSON.stringify(user));
}

export function loadSession(): SessionUser | null {
  try {
    const raw = window.localStorage.getItem(KEY);
    return raw ? JSON.parse(raw) as SessionUser : null;
  } catch {
    return null;
  }
}

export function clearSession() {
  window.localStorage.removeItem(KEY);
}
