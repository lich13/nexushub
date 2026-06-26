import { type RuntimeCapabilityInput } from "./runtimeViewModel";
import { slashCommandsForRuntime, type SlashCommand } from "./slashCommands";
import type { PluginInfo, UploadRecord } from "../../types";

export type ComposerUpload = UploadRecord & {
  local_status?: "uploading" | "ready" | "error";
  local_error?: string | null;
};

export type ComposerActionMode = "send" | "stop" | "followup" | "disabled";

export type PluginMentionCandidate = {
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
  if (key === "ArrowDown") return moveMenuSelection(current, total, 1);
  if (key === "ArrowUp") return moveMenuSelection(current, total, -1);
  return current;
}

function moveMenuSelection(current: number, total: number, delta: number): number {
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
