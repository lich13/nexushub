import { X } from "lucide-react";
import {
  formatFileSize,
  uploadKindLabel,
  uploadStatusText,
  type ComposerUpload
} from "../../lib/domain/composerViewModel";

export function ComposerAttachmentList({
  uploads,
  removingUploadId,
  onRemove
}: {
  uploads: ComposerUpload[];
  removingUploadId?: string | null;
  onRemove: (upload: ComposerUpload) => void;
}) {
  if (uploads.length === 0) return null;
  return (
    <div className="attachment-list" aria-label="已选择附件">
      {uploads.map((upload) => {
        const errored = upload.local_status === "error" || upload.status === "error";
        const uploading = upload.local_status === "uploading" || upload.status === "uploading";
        return (
          <div key={upload.id} className={errored ? "attachment-chip error" : uploading ? "attachment-chip uploading" : "attachment-chip"}>
            <div className="attachment-copy">
              <strong title={upload.name}>{upload.name}</strong>
              <small>{uploadKindLabel(upload.kind)} · {formatFileSize(upload.size)} · {uploadStatusText(upload)}</small>
            </div>
            <button
              type="button"
              className="icon-button compact attachment-remove"
              onClick={() => onRemove(upload)}
              disabled={removingUploadId === upload.id}
              title="移除附件"
            >
              <X size={14} />
            </button>
          </div>
        );
      })}
    </div>
  );
}
