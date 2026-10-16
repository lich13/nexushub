import { useCallback, useEffect, useRef, useState } from "react";
import { actOnDesktopThread, attachDesktopThread, openDesktopThread, type DesktopAction, type DesktopView } from "../lib/query/desktop";
import { beforeMachineChange, connectionSnapshot, useConnection } from "../lib/query/connection";
import { useThreadCacheActions } from "../lib/query/threads";

export function useCodexDesktop(threadId: string) {
  const connection = useConnection();
  const cache = useThreadCacheActions();
  const [view, setView] = useState<DesktopView | null>(null);
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  const [attempt, setAttempt] = useState(0);
  const [visible, setVisible] = useState(() => !document.hidden);
  const lifetime = useRef(0);
  const current = useRef<DesktopView | null>(null);
  const events = useRef(0);
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
    current.current = null; setView(null); setError(""); setBusy(false);
    if (switched || !attempt || connection.target !== "local" || !visible) return;
    let disposed = false;
    let detach: (() => Promise<void>) | undefined;
    let refresh: ReturnType<typeof setTimeout> | undefined;
    const valid = () => !disposed && generation === lifetime.current && connectionSnapshot().revision === connection.revision;
    const receive = (next: DesktopView) => {
      if (!valid() || next.threadId !== threadId) return;
      const previous = current.current;
      events.current += 1;
      current.current = next; setView(next);
      if (!refresh) refresh = setTimeout(() => {
        refresh = undefined;
        if (valid()) void cache.invalidateThread(threadId);
      }, 350);
      if (previous?.runtime !== next.runtime) void cache.invalidateThreads();
    };
    const cleanup = async () => {
      disposed = true;
      clearTimeout(refresh);
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
    setBusy(true);
    void attachDesktopThread(threadId, receive).then(async attached => {
      if (!valid()) { await attached.detach(); return; }
      detach = attached.detach; receive(attached.initial);
    }).catch(reason => { if (valid()) setError(reason instanceof Error ? reason.message : "无法连接 Codex 桌面"); })
      .finally(() => { if (valid()) setBusy(false); });
    return () => { unregister(); void cleanup(); };
  }, [threadId, connection.revision, connection.target, attempt, visible, cache]);

  const act = useCallback(async (action: DesktopAction) => {
    const captured = current.current;
    if (!captured?.connected || !captured.writable) throw new Error("请先连接 Codex 桌面");
    const generation = lifetime.current;
    const sequence = events.current;
    setBusy(true); setError("");
    try {
      const result = await actOnDesktopThread(captured.sessionId, action);
      if (generation !== lifetime.current || current.current?.sessionId !== captured.sessionId) throw new Error("所选线程已变化，旧操作结果已丢弃");
      // Stream callbacks may have advanced beyond the command's response.
      if (!current.current || result.revision > current.current.revision || (events.current === sequence && result.revision === current.current.revision)) {
        current.current = result; setView(result);
      }
      void cache.invalidateThreadAndThreads(threadId);
    } catch (reason) {
      if (generation === lifetime.current) setError(reason instanceof Error ? reason.message : "操作失败");
      throw reason;
    } finally { if (generation === lifetime.current) setBusy(false); }
  }, [cache, threadId]);

  const open = useCallback(async () => {
    setBusy(true); setError("");
    const generation = lifetime.current;
    try { await openDesktopThread(threadId); }
    catch (reason) { if (generation === lifetime.current) setError(reason instanceof Error ? reason.message : "无法打开 Codex"); }
    finally { if (generation === lifetime.current) setBusy(false); }
  }, [threadId]);

  return { view, error: error || view?.error || "", busy, act, open, connect: () => setAttempt(value => value + 1) };
}
