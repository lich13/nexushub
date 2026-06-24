import { X } from "lucide-react";
import { ChangeEvent, useEffect, useMemo, useRef, useState } from "react";
import { capabilitiesForInput, type RuntimeCapabilityInput } from "../../lib/domain/runtimeViewModel";
import { slashCommandsForRuntime, type SlashCommand } from "../../lib/domain/slashCommands";
import { useUploadActions } from "../../lib/query/threads";
import type { PluginInfo, UploadRecord } from "../../types";
import type { RuntimeCapabilityMatrix } from "../../lib/query/system";

export type ComposerUpload = UploadRecord & {
  local_status?: "uploading" | "ready" | "error";
  local_error?: string | null;
};

export type ComposerActionMode = "send" | "stop" | "followup" | "disabled";

type PluginMentionCandidate = {
  id: string;
  label: string;
  description: string;
  unavailableReason?: string | null;
  plugin?: PluginInfo;
};

type SlashQuery = {
  start: number;
  end: number;
  value: string;
};

type TriggerQuery = SlashQuery & {
  trigger: "/" | "@";
};

export function composerActionMode(running: boolean, draft: string, canStop: boolean, attachmentCount = 0): ComposerActionMode {
  const hasContent = draft.trim().length > 0 || attachmentCount > 0;
  if (running && hasContent) return "followup";
  if (running) return canStop ? "stop" : "disabled";
  return hasContent ? "send" : "disabled";
}

export function composerActionLabel(mode: ComposerActionMode): string {
  if (mode === "stop") return "停止";
  if (mode === "followup") return "跟进";
  if (mode === "send") return "发送";
  return "发送";
}

export function composerActionTitle(mode: ComposerActionMode): string {
  if (mode === "stop") return "停止当前运行中的 turn";
  if (mode === "followup") return "跟进当前 turn；不可用时自动加入跟进队列";
  if (mode === "send") return "发送新 turn";
  return "输入消息后发送";
}

export function readyComposerUploads(uploads: ComposerUpload[]): ComposerUpload[] {
  return uploads.filter((upload) => upload.status === "ready" && upload.local_status !== "error");
}

export function composerUploadIds(uploads: ComposerUpload[]): string[] {
  return readyComposerUploads(uploads).map((upload) => upload.id).filter(Boolean);
}

export function formatFileSize(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes < 0) return "unknown";
  if (bytes < 1024) return `${bytes} B`;
  const units = ["KiB", "MiB", "GiB"];
  let value = bytes / 1024;
  for (const unit of units) {
    if (value < 1024 || unit === units[units.length - 1]) {
      return `${value.toFixed(value >= 10 ? 0 : 1)} ${unit}`;
    }
    value /= 1024;
  }
  return `${bytes} B`;
}

export function uploadKindLabel(kind?: string | null): string {
  if (kind === "markdown") return "Markdown";
  if (kind === "spreadsheet") return "表格";
  if (kind === "document") return "文档";
  if (kind === "pdf") return "PDF";
  if (kind === "image") return "图片";
  if (kind === "file") return "文件";
  return "文本";
}

export function uploadStatusText(upload: Pick<ComposerUpload, "status" | "local_status" | "error_preview" | "local_error">): string {
  if (upload.local_status === "uploading" || upload.status === "uploading") return "上传中";
  if (upload.local_status === "error" || upload.status === "error") return upload.local_error || upload.error_preview || "上传失败";
  return "已就绪";
}

export function composerFileInputAcceptValue(): string | undefined {
  return undefined;
}

export function composerSubmitDraftValue(stateValue: string, domValue?: string | null): string {
  return typeof domValue === "string" ? domValue : stateValue;
}

function isInsideSimpleCodeContext(before: string): boolean {
  const backticks = (before.match(/`/g) ?? []).length;
  if (backticks % 2 === 1) return true;
  const singleQuotes = (before.match(/'/g) ?? []).length;
  const doubleQuotes = (before.match(/"/g) ?? []).length;
  return singleQuotes % 2 === 1 || doubleQuotes % 2 === 1;
}

function activeTriggerQuery(draft: string, cursor: number, trigger: "/" | "@"): TriggerQuery | null {
  const safeCursor = Math.max(0, Math.min(cursor, draft.length));
  const before = draft.slice(0, safeCursor);
  const start = before.lastIndexOf(trigger);
  if (start < 0) return null;
  if (start > 0 && !/\s/.test(before[start - 1])) return null;
  const value = before.slice(start);
  if (!value.startsWith(trigger) || value.includes("\n")) return null;
  if (trigger === "@" && isInsideSimpleCodeContext(before)) return null;
  return { start, end: safeCursor, value, trigger };
}

function activeSlashQuery(draft: string, cursor: number): SlashQuery | null {
  return activeTriggerQuery(draft, cursor, "/");
}

function activePluginMentionQuery(draft: string, cursor: number): SlashQuery | null {
  return activeTriggerQuery(draft, cursor, "@");
}

function nearestActiveComposerQuery(draft: string, cursor: number): TriggerQuery | null {
  const slash = activeTriggerQuery(draft, cursor, "/");
  const plugin = activeTriggerQuery(draft, cursor, "@");
  if (slash && plugin) return slash.start > plugin.start ? slash : plugin;
  return slash ?? plugin;
}

export function slashCommandSuggestions(draft: string, cursor: number, hasThread = true, desktop?: RuntimeCapabilityInput): SlashCommand[] {
  void hasThread;
  const query = activeSlashQuery(draft, cursor)?.value.toLowerCase();
  if (!query) return [];
  return slashCommandsForRuntime(desktop)
    .filter((item) => item.command.toLowerCase().startsWith(query));
}

export function applySlashCommandSelection(draft: string, cursor: number, command: string): { value: string; cursor: number } {
  const query = activeSlashQuery(draft, cursor);
  const insertion = `${command} `;
  if (!query) {
    const value = `${draft.slice(0, cursor)}${insertion}${draft.slice(cursor)}`;
    return { value, cursor: cursor + insertion.length };
  }
  const value = `${draft.slice(0, query.start)}${insertion}${draft.slice(query.end)}`;
  return { value, cursor: query.start + insertion.length };
}

export function pluginMentionSuggestions(
  draft: string,
  cursor: number,
  plugins: PluginInfo[] | null | undefined = [],
  unavailable = false
): PluginMentionCandidate[] {
  const query = activePluginMentionQuery(draft, cursor);
  if (!query) return [];
  const needle = query.value.slice(1).trim().toLowerCase();
  const rows = plugins ?? [];
  if (unavailable || rows.length === 0) {
    return [{
      id: "__plugins_unavailable__",
      label: "插件列表不可用",
      description: "当前无法读取插件列表",
      unavailableReason: "请稍后刷新，或在插件/Provider 页面查看可用能力。"
    }];
  }
  return rows
    .filter((plugin) => {
      if (!needle) return true;
      return plugin.id.toLowerCase().includes(needle) || plugin.label.toLowerCase().includes(needle);
    })
    .map((plugin) => ({
      id: plugin.id,
      label: plugin.label,
      description: plugin.description || plugin.kind || "插件能力",
      unavailableReason: plugin.unavailable_reason || (plugin.status === "planned" ? "当前能力尚未启用" : null),
      plugin
    }));
}

export function applyPluginMentionSelection(
  draft: string,
  cursor: number,
  plugin: Pick<PluginInfo, "id" | "label" | "invocation_template">
): { value: string; cursor: number } {
  const query = activePluginMentionQuery(draft, cursor);
  const label = (plugin.invocation_template || plugin.label || plugin.id).trim();
  const insertion = label.startsWith("@") ? `${label} ` : `@${label} `;
  if (!query) {
    const value = `${draft.slice(0, cursor)}${insertion}${draft.slice(cursor)}`;
    return { value, cursor: cursor + insertion.length };
  }
  const value = `${draft.slice(0, query.start)}${insertion}${draft.slice(query.end)}`;
  return { value, cursor: query.start + insertion.length };
}

export function exactSlashCommandFromDraft(draft: string, desktop?: RuntimeCapabilityInput): string | null {
  const command = draft.trim().replace(/\s+/g, " ");
  return slashCommandsForRuntime(desktop).some((item) => item.command === command) ? command : null;
}

export function slashCommandForComposerSubmit(draft: string, desktop?: RuntimeCapabilityInput): string | null {
  return exactSlashCommandFromDraft(draft, desktop);
}

export function activeComposerMenuKind(draft: string, cursor: number, plugins?: PluginInfo[] | null): "slash" | "plugin" | null {
  void plugins;
  const query = nearestActiveComposerQuery(draft, cursor);
  if (query?.trigger === "/") return "slash";
  if (query?.trigger === "@") return "plugin";
  return null;
}

export function nextSlashCommandSelection(current: number, total: number, key: string): number {
  if (key === "ArrowDown") return moveActionSelection(current, total, 1);
  if (key === "ArrowUp") return moveActionSelection(current, total, -1);
  return current;
}

function moveActionSelection(current: number, total: number, delta: number): number {
  if (total <= 0) return 0;
  return (current + delta + total) % total;
}

export function composerMenuKeyAction({
  key,
  shiftKey = false,
  composing = false,
  menuSelectionArmed = false,
  selected,
  suggestions
}: {
  key: string;
  shiftKey?: boolean;
  composing?: boolean;
  menuSelectionArmed?: boolean;
  selected: number;
  suggestions: Array<{ command?: string; id?: string }>;
}): { action: "move"; selected: number } | { action: "insert"; index: number } | { action: "dismiss" } | { action: "none" } {
  if (composing) return { action: "none" };
  if (key === "ArrowDown" || key === "ArrowUp") {
    return { action: "move", selected: nextSlashCommandSelection(selected, suggestions.length, key) };
  }
  if (key === "Escape") return { action: "dismiss" };
  if (key === "Tab" && suggestions.length > 0) {
    return { action: "insert", index: Math.min(Math.max(selected, 0), suggestions.length - 1) };
  }
  if (key === "Enter" && !shiftKey && menuSelectionArmed && suggestions.length > 0) {
    return { action: "insert", index: Math.min(Math.max(selected, 0), suggestions.length - 1) };
  }
  return { action: "none" };
}

export function slashCommandKeyAction({
  key,
  shiftKey = false,
  selected,
  suggestions
}: {
  key: string;
  shiftKey?: boolean;
  selected: number;
  suggestions: Array<{ command: string }>;
}): { action: "move"; selected: number } | { action: "insert"; command: string } | { action: "dismiss" } | { action: "none" } {
  if (key === "ArrowDown" || key === "ArrowUp") {
    return { action: "move", selected: nextSlashCommandSelection(selected, suggestions.length, key) };
  }
  if (key === "Escape") return { action: "dismiss" };
  if (key === "Enter" && !shiftKey && suggestions.length > 0) {
    return { action: "insert", command: suggestions[selected]?.command ?? suggestions[0].command };
  }
  return { action: "none" };
}

function escapeHtml(value: string): string {
  return value
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

export function renderSlashCommandMenuHtml(
  draft: string,
  cursor: number,
  hasThread = true,
  selected = 0,
  desktop?: RuntimeCapabilityInput,
): string {
  const suggestions = slashCommandSuggestions(draft, cursor, hasThread, desktop);
  if (suggestions.length === 0) return "";
  const options = suggestions.map((item, index) => {
    const className = index === selected ? "slash-option selected" : "slash-option";
    const threadBadge = item.requiresThread && !hasThread ? '<em class="slash-thread-note">需要已有线程</em>' : "";
    return [
      `<button type="button" class="${className}" role="option" aria-selected="${index === selected}">`,
      `<strong>${escapeHtml(item.command)}</strong>`,
      '<span class="slash-option-copy">',
      `<span>${escapeHtml(item.description)}</span>`,
      `<small>用法 ${escapeHtml(item.usageHint)}</small>`,
      threadBadge,
      "</span>",
      "</button>"
    ].join("");
  }).join("");
  return `<div class="slash-menu" role="listbox" aria-label="Slash 命令">${options}</div>`;
}

export function renderPluginMentionMenuHtml(
  draft: string,
  cursor: number,
  plugins: PluginInfo[] | null | undefined = [],
  unavailable = false,
  selected = 0
): string {
  const suggestions = pluginMentionSuggestions(draft, cursor, plugins, unavailable);
  if (suggestions.length === 0) return "";
  const options = suggestions.map((item, index) => {
    const className = index === selected ? "slash-option selected" : "slash-option";
    const reason = item.unavailableReason ? `<em>${escapeHtml(item.unavailableReason)}</em>` : "";
    return [
      `<button type="button" class="${className}" role="option" aria-selected="${index === selected}">`,
      `<strong>@${escapeHtml(item.label)}</strong>`,
      '<span class="slash-option-copy">',
      `<span>${escapeHtml(item.description)}</span>`,
      reason,
      "</span>",
      "</button>"
    ].join("");
  }).join("");
  return `<div class="slash-menu" role="listbox" aria-label="@ 插件">${options}</div>`;
}

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
