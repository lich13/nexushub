import { useEffect, useRef, useState } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { executeSessionBatch, previewSessionBatch, type SessionBatchPreview, type SessionBatchResult, type SessionOperation, type SessionProvider } from "../api/sessions";

export function useSessionSelection(provider: SessionProvider, filterKey: string, keys: string[], csrfToken?: string | null, onSucceeded?: (keys: string[], operation: SessionOperation) => void) {
  const client = useQueryClient();
  const [selecting, setSelecting] = useState(false);
  const [selection, setSelection] = useState<string[]>([]);
  const [preview, setPreview] = useState<SessionBatchPreview | null>(null);
  const [result, setResult] = useState<SessionBatchResult | null>(null);
  const scope = `${provider}\0${filterKey}`;
  const epoch = useRef({ scope, value: 0 });
  if (epoch.current.scope !== scope) epoch.current = { scope, value: epoch.current.value + 1 };
  const prepare = useMutation({
    mutationFn: async (operation: SessionOperation) => {
      const generation = epoch.current.value;
      return { generation, response: await previewSessionBatch({ provider, operation, sessionKeys: selection }, csrfToken) };
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
      const response = await executeSessionBatch({ provider: request.provider, operation: request.operation, items: request.items.filter(i => i.allowed && i.fingerprint).map(i => ({ sessionKey: i.sessionKey, fingerprint: i.fingerprint! })), confirmed: true }, csrfToken);
      return { response, generation, operation: request.operation };
    },
    onSuccess: ({ response, generation, operation }) => {
      const succeeded = response.items.filter(i => i.status === "succeeded").map(i => i.sessionKey);
      onSucceeded?.(succeeded, operation);
      if (generation === epoch.current.value) {
        setResult(response);
        setSelection(current => current.filter(key => !succeeded.includes(key)));
      }
      void client.invalidateQueries({ predicate: query => provider === "codex" ? /thread|system-status|probe-status/.test(String(query.queryKey[0])) : query.queryKey[0] === provider });
    }
  });
  useEffect(() => { setSelection([]); setPreview(null); setResult(null); setSelecting(false); prepare.reset(); execute.reset(); }, [provider, filterKey]);
  const visibleKeys = keys.join("\0");
  useEffect(() => { const visible = new Set(keys); setSelection(current => current.filter(key => visible.has(key))); }, [visibleKeys]);
  return {
    selecting, selection, preview, result, prepare, execute,
    busy: prepare.isPending || execute.isPending,
    toggle: (key: string) => { setPreview(null); setSelection(current => current.includes(key) ? current.filter(k => k !== key) : current.length < 100 ? [...current, key] : current); },
    begin: () => { setSelecting(true); setResult(null); },
    cancel: () => { epoch.current.value += 1; setSelecting(false); setSelection([]); setPreview(null); setResult(null); prepare.reset(); execute.reset(); },
    selectAll: () => { setPreview(null); setSelection(keys.slice(0, 100)); },
    clear: () => { setPreview(null); setSelection([]); },
    dismissPreview: () => { epoch.current.value += 1; setPreview(null); },
  };
}

export type SessionSelection = ReturnType<typeof useSessionSelection>;

export type { SessionOperation } from "../api/sessions";
