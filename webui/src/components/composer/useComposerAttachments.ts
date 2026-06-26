import { ChangeEvent, useMemo, useRef, useState } from "react";
import {
  readyComposerUploads,
  type ComposerUpload
} from "../../lib/domain/composerViewModel";
import { useUploadActions } from "../../lib/query/threads";

export function useComposerAttachments(csrfToken?: string | null, setFeedback?: (message: string | null) => void) {
  const inputRef = useRef<HTMLInputElement | null>(null);
  const [uploads, setUploads] = useState<ComposerUpload[]>([]);
  const [uploadInProgress, setUploadInProgress] = useState(false);
  const [removingUploadId, setRemovingUploadId] = useState<string | null>(null);
  const uploadActions = useUploadActions({ csrfToken });
  const readyUploads = useMemo(() => readyComposerUploads(uploads), [uploads]);

  const openPicker = () => {
    if (!uploadInProgress) inputRef.current?.click();
  };

  const onFileInputChange = async (event: ChangeEvent<HTMLInputElement>) => {
    const files = Array.from(event.target.files ?? []);
    event.target.value = "";
    if (files.length === 0) return;

    const pending = files.map((file, index): ComposerUpload => ({
      id: `uploading-${Date.now()}-${index}-${Math.random().toString(36).slice(2)}`,
      name: file.name || `file-${index + 1}`,
      mime: file.type || "application/octet-stream",
      size: file.size,
      sha256: "",
      kind: "text",
      status: "uploading",
      local_status: "uploading"
    }));
    const pendingIds = new Set(pending.map((item) => item.id));
    setUploads((current) => [...current, ...pending]);
    setUploadInProgress(true);
    setFeedback?.("正在上传附件...");

    try {
      const outcome = await uploadActions.upload(files);
      setUploads((current) => [
        ...current.filter((item) => !pendingIds.has(item.id)),
        ...outcome.files.map((file) => ({ ...file, local_status: "ready" as const }))
      ]);
      setFeedback?.(`已上传 ${outcome.files.length} 个附件`);
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      setUploads((current) => current.map((item) => pendingIds.has(item.id)
        ? {
          ...item,
          status: "error",
          local_status: "error",
          local_error: message,
          error_preview: message
        }
        : item));
      setFeedback?.(message);
    } finally {
      setUploadInProgress(false);
    }
  };

  const removeUpload = async (upload: ComposerUpload) => {
    setUploads((current) => current.filter((item) => item.id !== upload.id));
    if (upload.status !== "ready") return;
    setRemovingUploadId(upload.id);
    try {
      await uploadActions.delete(upload.id);
    } catch (error) {
      setFeedback?.(error instanceof Error ? error.message : String(error));
    } finally {
      setRemovingUploadId(null);
    }
  };

  const clearUploads = () => setUploads([]);

  return {
    inputRef,
    uploads,
    readyUploads,
    uploadInProgress,
    removingUploadId,
    openPicker,
    onFileInputChange,
    removeUpload,
    clearUploads
  };
}
