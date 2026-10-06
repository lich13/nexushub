import { useEffect, useRef, useState } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { executeSessionBatch, previewSessionBatch, type SessionBatchPreview, type SessionBatchResult, type SessionOperation, type SessionProvider } from "../api/sessions";

export function useSessionSelection(provider: SessionProvider, filterKey: string, keys: string[], onSucceeded?: (keys: string[], operation: SessionOperation) => void) {
  const client = useQueryClient();
  const returnFocus = useRef<HTMLElement | null>(null);
  const [selecting, setSelecting] = useState(false);
  const [selection, setSelection] = useState<string[]>([]);
  const [preview, setPreview] = useState<SessionBatchPreview | null>(null);
  const [result, setResult] = useState<SessionBatchResult | null>(null);
  const scope = `${provider}\0${filterKey}`;
  const epoch = useRef({ scope, value: 0 });
  if (epoch.current.scope !== scope) epoch.current = { scope, value: epoch.current.value + 1 };
  const prepare = useMutation({
    mutationFn: async (input: SessionOperation | { operation: SessionOperation; keys: string[] }) => {
      const operation = typeof input === "string" ? input : input.operation;
      const sessionKeys = typeof input === "string" ? selection : input.keys;
      const generation = epoch.current.value;
      return { generation, response: await previewSessionBatch({ provider, operation, sessionKeys }) };
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
      const response = await executeSessionBatch({ provider: request.provider, operation: request.operation, items: request.items.filter(i => i.allowed && i.fingerprint).map(i => ({ sessionKey: i.sessionKey, fingerprint: i.fingerprint! })), confirmed: true });
      return { response, generation, operation: request.operation };
    },
    onSuccess: ({ response, generation, operation }) => {
      const succeeded = response.items.filter(i => i.status === "succeeded").map(i => i.sessionKey);
      if (generation === epoch.current.value) {
        onSucceeded?.(succeeded, operation);
        setResult(response);
        setSelection(current => current.filter(key => !succeeded.includes(key)));
      }
      void client.invalidateQueries({ predicate: query => provider === "codex" ? /thread|system-capabilities|probe-status/.test(String(query.queryKey[0])) : query.queryKey[0] === (provider === "claude_code" ? "claude" : provider) });
    }
  });
  useEffect(() => { setSelection([]); setPreview(null); setResult(null); setSelecting(false); prepare.reset(); execute.reset(); }, [provider, filterKey]);
  const visibleKeys = keys.join("\0");
  useEffect(() => { const visible = new Set(keys); setSelection(current => current.filter(key => visible.has(key))); }, [visibleKeys]);
  return {
    selecting, selection, preview, result, prepare, execute, returnFocus,
    busy: prepare.isPending || execute.isPending,
    toggle: (key: string) => { setPreview(null); setSelection(current => current.includes(key) ? current.filter(k => k !== key) : current.length < 100 ? [...current, key] : current); },
    begin: () => { setSelecting(true); setResult(null); },
    prepareSingle: (key: string, operation: SessionOperation, origin?: HTMLElement | null) => {
      returnFocus.current = origin ?? (document.activeElement instanceof HTMLElement ? document.activeElement : null);
      epoch.current.value += 1;
      setSelection([key]);
      setResult(null);
      setPreview(null);
      prepare.mutate({ operation, keys: [key] });
    },
    cancel: () => { epoch.current.value += 1; setSelecting(false); setSelection([]); setPreview(null); setResult(null); prepare.reset(); execute.reset(); },
    selectAll: () => { setPreview(null); setSelection(keys.slice(0, 100)); },
    clear: () => { setPreview(null); setSelection([]); },
    dismissPreview: () => { epoch.current.value += 1; setPreview(null); },
  };
}

export type SessionSelection = ReturnType<typeof useSessionSelection>;

export type { SessionOperation } from "../api/sessions";
