import { useCallback, useEffect, useRef, useState } from "react";
import { actOnDesktopThread, attachDesktopThread, openDesktopThread, type DesktopAction, type DesktopView } from "../lib/query/desktop";
import { beforeMachineChange, connectionSnapshot, useConnection } from "../lib/query/connection";
import { useThreadCacheActions } from "../lib/query/threads";
import { consumeDesktopOpenIntent } from "../lib/desktopIntent";
import { desktopRefresh } from "../lib/desktopRefresh";

export function useCodexDesktop(threadId: string) {
  const connection = useConnection();
  const cache = useThreadCacheActions();
  const [view, setView] = useState<DesktopView | null>(null);
  const [error, setError] = useState("");
  const actionError = useRef(false);
  const [busy, setBusy] = useState(false);
  const [attempt, setAttempt] = useState(0);
  const [visible, setVisible] = useState(() => !document.hidden);
  const lifetime = useRef(0);
  const current = useRef<DesktopView | null>(null);
  const events = useRef(0);
  const confirmOwner = useRef(false);
  const refreshThread = useRef<(() => void) | null>(null);
  const previousThread = useRef(threadId);

  useEffect(() => {
    const changed = () => setVisible(!document.hidden);
    document.addEventListener("visibilitychange", changed);
    return () => document.removeEventListener("visibilitychange", changed);
  }, []);

  useEffect(() => {
    const generation = ++lifetime.current;
    const switched = previousThread.current !== threadId;
    if (switched) { previousThread.current = threadId; setAttempt(0); }
    current.current = null; setView(null); setError(""); actionError.current = false; setBusy(false);
    if (switched || !attempt || connection.target !== "local" || !visible) return;
    let disposed = false;
    let detach: (() => Promise<void>) | undefined;
    let sessionId: string | undefined;
    const retiredSessions = new Set<string>();
    const valid = () => !disposed && !document.hidden && generation === lifetime.current && connectionSnapshot().revision === connection.revision;
    const refresh = desktopRefresh(() => cache.refreshDesktopThread(threadId), valid);
    refreshThread.current = refresh.request;
    const receive = (next: DesktopView) => {
      if (!valid() || next.threadId !== threadId || retiredSessions.has(next.sessionId)) return;
      const changedSession = sessionId !== undefined && next.sessionId !== sessionId;
      if (changedSession) {
        // Only the ordered current native channel can rotate an epoch. Older
        // services without recovery metadata retain the strict session guard.
        if (!next.connectionPhase) return;
        retiredSessions.add(sessionId!);
        setError(""); setBusy(false);
      }
      sessionId = next.sessionId;
      const previous = current.current;
      if (!changedSession && previous && next.revision < previous.revision) return;
      events.current += 1;
      current.current = next; setView(next);
      if (changedSession || previous?.connectionPhase !== next.connectionPhase) setError("");
      refresh.request();
      if (previous?.runtime !== next.runtime) void cache.invalidateThreads();
    };
    const cleanup = async () => {
      disposed = true;
      refresh.dispose();
      if (refreshThread.current === refresh.request) refreshThread.current = null;
      const release = detach; detach = undefined;
      if (release) await release().catch(() => {});
    };
    const unregister = beforeMachineChange(async () => {
      // A failed machine change can leave target/revision unchanged. Clear the
      // detached selection now so that case still exposes an explicit reconnect.
      ++lifetime.current;
      current.current = null; setView(null); setBusy(false); setAttempt(0);
      await cleanup();
    });
    const confirm = confirmOwner.current;
    confirmOwner.current = false;
    setBusy(true);
    void attachDesktopThread(threadId, receive, confirm).then(async attached => {
      if (!valid()) { await attached.detach(); return; }
      detach = attached.detach; receive(attached.initial);
    }).catch(reason => { if (valid()) { actionError.current = false; setError(reason instanceof Error ? reason.message : "无法连接 Codex 桌面"); } })
      .finally(() => { if (valid()) setBusy(false); });
    return () => { unregister(); void cleanup(); };
  }, [threadId, connection.revision, connection.target, attempt, visible, cache]);

  useEffect(() => {
    if (consumeDesktopOpenIntent(threadId)) { confirmOwner.current = true; setAttempt(value => value + 1); }
  }, [threadId]);

  const act = useCallback(async (action: DesktopAction) => {
    const captured = current.current;
    if (!captured?.connected || !captured.writable) throw new Error("请先连接 Codex 桌面");
    const generation = lifetime.current;
    const sequence = events.current;
    setBusy(true); setError(""); actionError.current = false;
    try {
      const result = await actOnDesktopThread(captured.sessionId, action);
      if (generation !== lifetime.current || current.current?.sessionId !== captured.sessionId) throw new Error("所选线程已变化，旧操作结果已丢弃");
      // Stream callbacks may have advanced beyond the command's response.
      if (!current.current || result.revision > current.current.revision || (events.current === sequence && result.revision === current.current.revision)) {
        const updated = { ...result, desktopBuild: result.desktopBuild ?? current.current?.desktopBuild };
        current.current = updated; setView(updated);
      }
      refreshThread.current?.();
      void cache.invalidateThreads();
    } catch (reason) {
      if (generation === lifetime.current && current.current?.sessionId === captured.sessionId) {
        actionError.current = true; setError(reason instanceof Error ? reason.message : "操作失败");
      }
      throw reason;
    } finally { if (generation === lifetime.current && current.current?.sessionId === captured.sessionId) setBusy(false); }
  }, [cache, threadId]);

  const open = useCallback(async () => {
    setBusy(true); setError(""); actionError.current = false;
    const generation = lifetime.current;
    try {
      await openDesktopThread(threadId);
      if (generation === lifetime.current && !document.hidden) { confirmOwner.current = true; setAttempt(value => value + 1); }
    }
    catch (reason) { if (generation === lifetime.current) setError(reason instanceof Error ? reason.message : "无法打开 Codex"); }
    finally { if (generation === lifetime.current) setBusy(false); }
  }, [threadId]);

  const clearActionError = useCallback(() => {
    // Editing a reference only dismisses the last action's local feedback.
    // Native view errors and unknown queue entries remain authoritative.
    if (actionError.current) { actionError.current = false; setError(""); }
  }, []);
  return { view, error: error || view?.error || "", busy, act, open, clearActionError, connect: () => { confirmOwner.current = true; setAttempt(value => value + 1); } };
}
