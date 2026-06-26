import { X } from "lucide-react";
import { ChangeEvent, useEffect, useMemo, useRef, useState } from "react";
import {
  activeComposerMenuKind,
  applyPluginMentionSelection,
  applySlashCommandSelection,
  composerMenuKeyAction,
  exactSlashCommandFromDraft,
  formatFileSize,
  pluginMentionSuggestions,
  readyComposerUploads,
  slashCommandSuggestions,
  uploadKindLabel,
  uploadStatusText,
  type ComposerUpload,
  type PluginMentionCandidate
} from "../../lib/domain/composerViewModel";
import { capabilitiesForInput } from "../../lib/domain/runtimeViewModel";
import type { SlashCommand } from "../../lib/domain/slashCommands";
import { useUploadActions } from "../../lib/query/threads";
import type { PluginInfo } from "../../types";
import type { RuntimeCapabilityMatrix } from "../../lib/query/system";

export type { ComposerActionMode, ComposerUpload, PluginMentionCandidate } from "../../lib/domain/composerViewModel";
export {
  activeComposerMenuKind,
  applyPluginMentionSelection,
  applySlashCommandSelection,
  composerActionLabel,
  composerActionMode,
  composerActionTitle,
  composerFileInputAcceptValue,
  composerMenuKeyAction,
  composerSubmitDraftValue,
  composerUploadIds,
  exactSlashCommandFromDraft,
  formatFileSize,
  pluginMentionSuggestions,
  readyComposerUploads,
  renderPluginMentionMenuHtml,
  renderSlashCommandMenuHtml,
  nextSlashCommandSelection,
  slashCommandForComposerSubmit,
  slashCommandKeyAction,
  slashCommandSuggestions,
  uploadKindLabel,
  uploadStatusText
} from "../../lib/domain/composerViewModel";

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

export function SlashCommandTextarea({
  inputRef,
  value,
  onChange,
  placeholder,
  hasThread,
  plugins,
  pluginsUnavailable = false,
  capabilities = capabilitiesForInput(),
  onSlashCommand,
  onSubmitShortcut,
  disabled = false
}: {
  inputRef?: (node: HTMLTextAreaElement | null) => void;
  value: string;
  onChange: (value: string) => void;
  placeholder: string;
  hasThread: boolean;
  plugins?: PluginInfo[] | null;
  pluginsUnavailable?: boolean;
  capabilities?: RuntimeCapabilityMatrix;
  onSlashCommand?: (command: string) => void;
  onSubmitShortcut?: (value?: string | null) => void;
  disabled?: boolean;
}) {
  const textareaRef = useRef<HTMLTextAreaElement | null>(null);
  const [cursor, setCursor] = useState(0);
  const [selected, setSelected] = useState(0);
  const [menuSelectionArmed, setMenuSelectionArmed] = useState(false);
  const [dismissedSignature, setDismissedSignature] = useState<string | null>(null);
  const signature = `${value}:${cursor}`;
  const menuKind = activeComposerMenuKind(value, cursor, plugins);
  const slashSuggestions = menuKind === "slash" ? slashCommandSuggestions(value, cursor, hasThread, capabilities) : [];
  const pluginSuggestions = menuKind === "plugin" ? pluginMentionSuggestions(value, cursor, plugins, pluginsUnavailable) : [];
  const suggestions = dismissedSignature === signature ? [] : menuKind === "plugin" ? pluginSuggestions : slashSuggestions;
  const open = suggestions.length > 0;
  const ariaLabel = menuKind === "plugin" ? "@ 插件" : "Slash 命令";
  const updateCursor = (target: HTMLTextAreaElement) => setCursor(target.selectionStart ?? target.value.length);
  const insertCommand = (command: string) => {
    const next = applySlashCommandSelection(value, cursor, command);
    onChange(next.value);
    setCursor(next.cursor);
    setSelected(0);
    setMenuSelectionArmed(false);
    requestAnimationFrame(() => {
      const textarea = textareaRef.current;
      if (!textarea) return;
      textarea.focus();
      textarea.setSelectionRange(next.cursor, next.cursor);
    });
  };
  const insertPlugin = (candidate: PluginMentionCandidate) => {
    if (candidate.id === "__plugins_unavailable__") return;
    const plugin = candidate.plugin ?? { id: candidate.id, label: candidate.label, status: "ready", kind: "builtin" };
    const next = applyPluginMentionSelection(value, cursor, plugin);
    onChange(next.value);
    setCursor(next.cursor);
    setSelected(0);
    setMenuSelectionArmed(false);
    requestAnimationFrame(() => {
      const textarea = textareaRef.current;
      if (!textarea) return;
      textarea.focus();
      textarea.setSelectionRange(next.cursor, next.cursor);
    });
  };
  const maybeRunExactSlashCommand = (currentValue = value) => {
    if (!onSlashCommand) return false;
    const command = exactSlashCommandFromDraft(currentValue, capabilities);
    if (!command) return false;
    onSlashCommand(command);
    return true;
  };
  const selectedSlashMatchesExactDraft = (command: string, currentValue = value) => exactSlashCommandFromDraft(currentValue, capabilities) === command;

  useEffect(() => {
    if (selected >= suggestions.length) {
      setSelected(0);
      setMenuSelectionArmed(false);
    }
  }, [selected, suggestions.length]);

  return (
    <div className="slash-composer">
      {open && (
        <div className="slash-menu" role="listbox" aria-label={ariaLabel}>
          {suggestions.map((item, index) => (
            <button
              key={menuKind === "plugin" ? (item as PluginMentionCandidate).id : (item as SlashCommand).command}
              type="button"
              className={index === selected ? "slash-option selected" : "slash-option"}
              onMouseDown={(event) => {
                event.preventDefault();
                if (menuKind === "plugin") {
                  insertPlugin(item as PluginMentionCandidate);
                } else {
                  insertCommand((item as SlashCommand).command);
                }
              }}
              role="option"
              aria-selected={index === selected}
            >
              <strong>{menuKind === "plugin" ? `@${(item as PluginMentionCandidate).label}` : (item as SlashCommand).command}</strong>
              <span className="slash-option-copy">
                <span>{item.description}</span>
                {menuKind === "plugin" ? null : <small>用法 {(item as SlashCommand).usageHint}</small>}
                {menuKind === "plugin" && (item as PluginMentionCandidate).unavailableReason ? <em>{(item as PluginMentionCandidate).unavailableReason}</em> : null}
                {menuKind !== "plugin" && (item as SlashCommand).requiresThread && !hasThread ? <em>需要已有线程</em> : null}
              </span>
            </button>
          ))}
        </div>
      )}
      <textarea
        ref={(node) => {
          textareaRef.current = node;
          inputRef?.(node);
        }}
        value={value}
        disabled={disabled}
        onChange={(event) => {
          onChange(event.target.value);
          setDismissedSignature(null);
          setMenuSelectionArmed(false);
          updateCursor(event.target);
        }}
        onClick={(event) => {
          setDismissedSignature(null);
          setMenuSelectionArmed(false);
          updateCursor(event.currentTarget);
        }}
        onKeyUp={(event) => {
          if (event.key !== "Escape") setDismissedSignature(null);
          updateCursor(event.currentTarget);
        }}
        onKeyDown={(event) => {
          if (!open) {
            if (event.nativeEvent.isComposing) return;
            if (event.key === "Enter" && !event.shiftKey) {
              event.preventDefault();
              if (!maybeRunExactSlashCommand(event.currentTarget.value)) {
                onSubmitShortcut?.(event.currentTarget.value);
              }
            }
            return;
          }
          const action = composerMenuKeyAction({
            key: event.key,
            shiftKey: event.shiftKey,
            composing: event.nativeEvent.isComposing,
            menuSelectionArmed,
            selected,
            suggestions
          });
          if (action.action === "move") {
            event.preventDefault();
            setSelected(action.selected);
            setMenuSelectionArmed(true);
          } else if (action.action === "dismiss") {
            event.preventDefault();
            setSelected(0);
            setMenuSelectionArmed(false);
            setDismissedSignature(signature);
          } else if (action.action === "insert") {
            event.preventDefault();
            const item = suggestions[action.index];
            if (!item) return;
            if (menuKind === "plugin") {
              insertPlugin(item as PluginMentionCandidate);
            } else {
              const command = (item as SlashCommand).command;
              if (selectedSlashMatchesExactDraft(command, event.currentTarget.value) && maybeRunExactSlashCommand(event.currentTarget.value)) {
                return;
              }
              insertCommand(command);
            }
          } else if (event.key === "Enter" && !event.shiftKey && !event.nativeEvent.isComposing) {
            event.preventDefault();
            if (!maybeRunExactSlashCommand(event.currentTarget.value)) {
              onSubmitShortcut?.(event.currentTarget.value);
            }
          }
        }}
        placeholder={placeholder}
      />
    </div>
  );
}
