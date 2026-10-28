import { useEffect, useRef, useState } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { executeSessionBatch, previewSessionBatch, type SessionBatchPreview, type SessionBatchResult, type SessionOperation, type SessionProvider } from "../api/sessions";
import { useConnection } from "./connection";
import { addSessionSelection, clickSessionSelection, emptySessionSelection, syncSessionSelection, type SessionClick } from "../domain/sessionSelection";

export function useSessionSelection(provider: SessionProvider, filterKey: string, keys: string[], onSucceeded?: (keys: string[], operation: SessionOperation) => void) {
  const client = useQueryClient();
  const connection = useConnection();
  const scope = `${connection.revision}\0${connection.target}\0${provider}\0${filterKey}`;
  const returnFocus = useRef<HTMLElement | null>(null);
  const [state, setState] = useState(() => emptySessionSelection(scope));
  const current = syncSessionSelection(state, scope, keys);
  const { selecting, keys: selection, notice } = current;
  const [preview, setPreview] = useState<SessionBatchPreview | null>(null);
  const [result, setResult] = useState<SessionBatchResult | null>(null);
  const epoch = useRef({ scope, value: 0 });
  if (epoch.current.scope !== scope) epoch.current = { scope, value: epoch.current.value + 1 };
  const prepare = useMutation({
    mutationFn: async (input: SessionOperation | { operation: SessionOperation; keys: string[] }) => {
      const operation = typeof input === "string" ? input : input.operation;
      const sessionKeys = typeof input === "string" ? selection : input.keys;
      const generation = epoch.current.value;
      const includeDescendants = (provider === "codex" || provider === "antigravity") && operation === "delete";
      if (generation !== epoch.current.value) throw new Error("选择范围已变化，请重新预览");
      const response: SessionBatchPreview = { provider, operation, items: [] };
      const size = operation === "delete" ? Math.max(1, sessionKeys.length) : 100;
      for (let start = 0; start < sessionKeys.length; start += size) {
        if (generation !== epoch.current.value) throw new Error("选择范围已变化");
        const page = await previewSessionBatch({ provider, operation, sessionKeys: sessionKeys.slice(start, start + size), ...(includeDescendants ? { includeDescendants: true } : {}) });
        response.items.push(...page.items);
      }
      return { generation, response };
    },
    onSuccess: ({ generation, response }) => { if (generation === epoch.current.value) setPreview(response); }
  });
  const execute = useMutation({
    mutationFn: async () => {
      if (!preview) throw new Error("请先重新预览");
      const generation = epoch.current.value;
      const request = preview;
      // Every attempt consumes its preview, including an uncertain transport failure.
      setPreview(null);
      const items = request.items.filter(i => i.allowed).map(i => ({ sessionKey: i.sessionKey, ...(request.operation === "delete" ? { descendants: i.descendants } : { fingerprint: i.fingerprint! }) }));
      const response: SessionBatchResult = { items: [] };
      const size = request.operation === "delete" ? Math.max(1, items.length) : 100;
      for (let start = 0; start < items.length; start += size) {
        if (generation !== epoch.current.value) throw new Error("选择范围已变化");
        const result = await executeSessionBatch({ provider: request.provider, operation: request.operation, items: items.slice(start, start + size), confirmed: true, ...((request.provider === "codex" || request.provider === "antigravity") && request.operation === "delete" ? { includeDescendants: true } : {}) });
        response.items.push(...result.items);
      }
      return { response, generation, operation: request.operation };
    },
    onSuccess: ({ response, generation, operation }) => {
      const succeeded = [...new Set(response.items.flatMap(i => [...(i.status === "succeeded" ? [i.sessionKey] : []), ...(i.deletedIds ?? [])]))];
      if (generation === epoch.current.value) {
        onSucceeded?.(succeeded, operation);
        setResult(response);
        setState(current => ({ ...current, keys: current.keys.filter(key => !succeeded.includes(key)), anchor: current.anchor && succeeded.includes(current.anchor) ? null : current.anchor }));
      }
      void client.invalidateQueries({ predicate: query => provider === "codex" ? /thread|system-capabilities|probe-status/.test(String(query.queryKey[0])) : query.queryKey[0] === provider });
    }
  });
  useEffect(() => { setState(emptySessionSelection(scope)); setPreview(null); setResult(null); prepare.reset(); execute.reset(); }, [scope]);
  const visibleKeys = keys.join("\0");
  useEffect(() => { setState(current => syncSessionSelection(current, scope, keys)); }, [scope, visibleKeys]);
  const select = (key: string, event: SessionClick, open: () => void) => {
    if (prepare.isPending || execute.isPending || (event.ctrlKey && event.detail !== 0)) return;
    const modified = event.detail !== 0 && (event.metaKey || event.shiftKey);
    setState(clickSessionSelection(current, keys, key, event));
    if (selecting || modified) { epoch.current.value += 1; setPreview(null); setResult(null); }
    else open();
  };
  return {
    selecting, selection, notice, preview, result, prepare, execute, returnFocus, select,
    busy: prepare.isPending || execute.isPending,
    toggle: (key: string) => select(key, { detail: 1, metaKey: true, shiftKey: false, ctrlKey: false }, () => {}),
    begin: () => { setState({ ...current, selecting: true }); setResult(null); },
    prepareSingle: (key: string, operation: SessionOperation, origin?: HTMLElement | null) => {
      returnFocus.current = origin ?? (document.activeElement instanceof HTMLElement ? document.activeElement : null);
      epoch.current.value += 1;
      setState({ ...current, keys: [key], anchor: key, notice: null });
      setResult(null);
      setPreview(null);
      prepare.mutate({ operation, keys: [key] });
    },
    cancel: () => { epoch.current.value += 1; setState(emptySessionSelection(scope)); setPreview(null); setResult(null); prepare.reset(); execute.reset(); },
    selectAll: () => { epoch.current.value += 1; setPreview(null); setState(addSessionSelection(current, keys)); },
    clear: () => { epoch.current.value += 1; setPreview(null); setState({ ...current, keys: [], anchor: null, notice: null }); },
    dismissPreview: () => { epoch.current.value += 1; setPreview(null); },
  };
}

export type SessionSelection = ReturnType<typeof useSessionSelection>;

export type { SessionOperation } from "../api/sessions";
