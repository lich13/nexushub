import { useEffect, useRef, useState } from "react";
import {
  activeComposerMenuKind,
  applyPluginMentionSelection,
  applySlashCommandSelection,
  composerMenuKeyAction,
  exactSlashCommandFromDraft,
  pluginMentionSuggestions,
  slashCommandSuggestions,
  type PluginMentionCandidate
} from "../../lib/domain/composerViewModel";
import { capabilitiesForInput } from "../../lib/domain/runtimeViewModel";
import type { SlashCommand } from "../../lib/domain/slashCommands";
import type { PluginInfo } from "../../types";
import type { RuntimeCapabilityMatrix } from "../../lib/query/system";
export { ComposerAttachmentList } from "./ComposerAttachmentList";
export { useComposerAttachments } from "./useComposerAttachments";

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
