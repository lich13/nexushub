import { readSessionAttachment, type SessionAttachmentRequest } from "../api/sessions";

const cache = new Map<string, string>();
const pending = new Map<string, Promise<string>>();
const MAX_CACHE_CHARS = 32 * 1024 * 1024;
let chars = 0;
let generation = 0;

export function readAttachment(request: SessionAttachmentRequest): Promise<string> {
  const key = JSON.stringify(request);
  const cached = cache.get(key);
  if (cached) { cache.delete(key); cache.set(key, cached); return Promise.resolve(cached); }
  const active = pending.get(key);
  if (active) return active;
  const startedGeneration = generation;
  const promise = readSessionAttachment(request).then(result => {
    if (!["image/png", "image/jpeg", "image/webp", "image/gif"].includes(result.mimeType) || result.base64.length > 20 * 1024 * 1024 * 4 / 3 + 4) throw new Error("图片格式或大小不支持预览");
    const url = `data:${result.mimeType};base64,${result.base64}`;
    if (startedGeneration !== generation) return url;
    while (cache.size && (chars + url.length > MAX_CACHE_CHARS || cache.size >= 32)) {
      const first = cache.keys().next().value!;
      chars -= cache.get(first)!.length; cache.delete(first);
    }
    cache.set(key, url); chars += url.length;
    return url;
  }).finally(() => { if (pending.get(key) === promise) pending.delete(key); });
  pending.set(key, promise);
  return promise;
}

export function clearAttachmentCache() { generation++; pending.clear(); cache.clear(); chars = 0; }
if (typeof window !== "undefined") window.addEventListener("pagehide", clearAttachmentCache);
