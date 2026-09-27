import { afterEach, expect, test, vi } from "vitest";
import { readSessionAttachment } from "../api/sessions";
import { clearAttachmentCache, readAttachment } from "./attachments";

vi.mock("../api/sessions", () => ({ readSessionAttachment: vi.fn() }));
const request = { provider: "codex" as const, sessionKey: "thread", messageId: "message", attachmentId: "attachment" };
afterEach(() => { clearAttachmentCache(); vi.resetAllMocks(); });

test("concurrent reads and repeated polling reuse a single response", async () => {
  vi.mocked(readSessionAttachment).mockResolvedValue({ mimeType: "image/png", base64: "aGVsbG8=" });
  const [a, b] = await Promise.all([readAttachment(request), readAttachment(request)]);
  expect(a).toBe(b); expect(await readAttachment(request)).toBe(a);
  expect(readSessionAttachment).toHaveBeenCalledTimes(1);
});

test("a response arriving after logout cannot repopulate the cache", async () => {
  let resolve!: (value: { mimeType: string; base64: string }) => void;
  vi.mocked(readSessionAttachment).mockImplementationOnce(() => new Promise(done => { resolve = done; }));
  const pending = readAttachment(request); clearAttachmentCache();
  resolve({ mimeType: "image/png", base64: "aGVsbG8=" }); await pending;
  vi.mocked(readSessionAttachment).mockResolvedValue({ mimeType: "image/png", base64: "bmV3" });
  expect(await readAttachment(request)).toContain("bmV3"); expect(readSessionAttachment).toHaveBeenCalledTimes(2);
});

test("large previews evict older entries and failed reads remain retryable", async () => {
  vi.mocked(readSessionAttachment).mockRejectedValueOnce(new Error("unavailable"));
  await expect(readAttachment(request)).rejects.toThrow("unavailable");
  vi.mocked(readSessionAttachment).mockResolvedValue({ mimeType: "image/png", base64: "A".repeat(11 * 1024 * 1024) });
  for (let index = 0; index < 3; index++) await readAttachment({ ...request, attachmentId: String(index) });
  await readAttachment({ ...request, attachmentId: "0" });
  expect(readSessionAttachment).toHaveBeenCalledTimes(5);
});
