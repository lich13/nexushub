import { SessionSize } from "../common/SessionSize";
import { Archive, ArchiveRestore, Copy, MessageSquare, RefreshCw, Search, Trash2 } from "lucide-react";
import { useCallback, useMemo, useState } from "react";
import { threadDetailFromSlot, useConversationController } from "../../hooks/useConversationController";
import {
  isThreadListItemRunning,
  threadListItemPreviewText,
  threadListItemStatusText,
  threadListItemText,
  type SelectedThread,
  type View
} from "../../lib/domain/codexViewModel";
import type { RuntimeCapabilityMatrix } from "../../lib/query/system";
import type { ThreadSummary } from "../../types";
import { Conversation } from "./Conversation";
import { useSessionSelection } from "../../lib/query/sessions";
import { SessionBatchControls, SessionCheckbox } from "../common/SessionBatchControls";
import { RunningIndicator } from "../common/RunningIndicator";
import { RenameableSession } from "../common/RenameableSession";
import { useReadOnlyThreadActions } from "../../lib/query/threads";
import { getThread } from "../../lib/api/threads";
import { latestAssistantCopyText, threadResumeCommand } from "../../lib/domain/conversationViewModel";
import { sharedDisabledStates } from "../../lib/domain/visualContract";
import type { SessionSearchResult } from "../../types";
import { useSearchWorkspace } from "../common/SessionSearch";

export const statusTabs = [
  { id: "all", label: "全部" },
  { id: "running", label: "运行中" },
  { id: "reply-needed", label: "待回复" },
  { id: "recoverable", label: "异常" },
  { id: "archived", label: "归档" }
];



export function ChatWorkspace({ mobileThreadsOpen, setMobileThreadsOpen, setView, capabilities }: {
  mobileThreadsOpen: boolean;
  setMobileThreadsOpen: (open: boolean) => void;
  setView: (view: View) => void;
  capabilities: RuntimeCapabilityMatrix;
}) {
  const [status, setStatus] = useState("all");
  const [q, setQ] = useState("");
  const {
    threadCache,
    messageStore,
    threads,
    visibleThreads,
    resolvedSelected,
    selectedThreadSummary,
    selectedDetail,
    detailLoading,
    nextThreadAfterRemoval,
    selectThread
  } = useConversationController({
    status,
    q,
    setMobileThreadsOpen
  });

  const list = (
    <ThreadList
      status={status}
      q={q}
      setQ={setQ}
      setStatus={setStatus}
      threads={visibleThreads}
      selectedId={resolvedSelected}
      onSelect={selectThread}
      onRefresh={() => threadCache.invalidateThreads()}
      loading={threads.isLoading}
      error={threads.error?.message}
      onBatchSucceeded={(keys) => { for (const key of keys) threadCache.clearArchivedThreadClientState(messageStore, key); if (resolvedSelected && keys.includes(resolvedSelected)) selectThread(null); }}
    />
  );
  const selectedSlot = resolvedSelected ? messageStore.getSlot(resolvedSelected) : null;
  const selectedFallback = selectedThreadSummary ?? selectedSlot?.summary ?? null;
  const [pendingSearch, setPendingSearch] = useState<SessionSearchResult | null>(null);
  const selectSearchResult = useCallback((result: SessionSearchResult) => {
    selectThread(result.sessionKey);
    setPendingSearch(result);
  }, [selectThread]);
  useSearchWorkspace(useMemo(() => ({
    provider: "codex" as const,
    sessionKey: resolvedSelected,
    selectResult: selectSearchResult
  }), [resolvedSelected, selectSearchResult]));

  return (
    <div className={`chat-layout ${!mobileThreadsOpen && resolvedSelected ? "has-selection" : ""}`}>
      <aside className="thread-column">{list}</aside>
      <section className="conversation-column">
        {resolvedSelected && selectedFallback ? (
          <Conversation
            threadId={resolvedSelected}
            detail={selectedDetail ?? threadDetailFromSlot(resolvedSelected, selectedSlot ?? messageStore.getSlot(resolvedSelected), selectedFallback)}
            selectedSummary={selectedThreadSummary}
            archivedView={status === "archived"}
            slot={messageStore.getSlot(resolvedSelected)}
            messageStore={messageStore}
            onSelect={(id) => selectThread(id)}
            onPanelSelect={setView}
            nextThreadAfterArchive={nextThreadAfterRemoval}
            capabilities={capabilities}
            onBack={() => setMobileThreadsOpen(true)}
            searchTarget={pendingSearch}
            searchReady={Boolean(selectedDetail || selectedSlot?.blocks.length)}
            onSearchResolved={() => setPendingSearch(null)}
            onSearchMiss={() => { setPendingSearch(null); }}
          />
        ) : (
          <div className="empty-state"><MessageSquare size={28} /><strong>{detailLoading ? "正在读取任务" : "选择一个任务"}</strong></div>
        )}
      </section>
    </div>
  );
}

function ThreadList({ status, q, setQ, setStatus, threads, selectedId, onSelect, onRefresh, loading, error, onBatchSucceeded }: {
  status: string;
  q: string;
  setQ: (value: string) => void;
  setStatus: (value: string) => void;
  threads: ThreadSummary[];
  selectedId: string | null;
  onSelect: (id: SelectedThread) => void;
  onRefresh: () => void;
  loading: boolean;
  error?: string;
  onBatchSucceeded: (keys: string[]) => void;
}) {
  const batch = useSessionSelection("codex", `${status}\0${q}`, threads.map(t => t.id), onBatchSucceeded);
  const rename = useReadOnlyThreadActions({});
  const [menuFeedback, setMenuFeedback] = useState("");
  const copy = async (value: string | null | undefined) => {
    if (!value) { setMenuFeedback("没有可复制的内容"); return; }
    try { await navigator.clipboard.writeText(value); setMenuFeedback("已复制"); }
    catch { setMenuFeedback("复制失败：剪贴板不可用"); }
  };
  return (
    <div className="thread-list">
      <div className="section-title thread-title-row">
        <div>
          <strong>Codex</strong>
        </div>
        <div className="thread-title-actions">
          <button className="icon-button compact" onClick={onRefresh} title="刷新任务"><RefreshCw size={16} /></button>
        </div>
      </div>
      <label className="search-box">
        <Search size={16} />
        <input value={q} onChange={(event) => setQ(event.target.value)} placeholder="搜索标题或 ID" />
      </label>
      <div className="segmented">
        {statusTabs.map((tab) => (
          <button key={tab.id} className={status === tab.id ? "active" : ""} onClick={() => setStatus(tab.id)}>{tab.label}</button>
        ))}
      </div>
      <div className="thread-scroll">
        <SessionBatchControls batch={batch} operations={status === "archived" ? ["restore", "delete"] : ["archive", "delete"]} />
        {error && <div className="form-error" role="alert">{error}</div>}
        {loading && <div className="muted-row">正在读取 Codex 状态...</div>}
        {threads.map((thread) => {
          const title = threadListItemText(thread);
          const preview = threadListItemPreviewText(thread);
          const running = isThreadListItemRunning(thread);
          return (
            <div className="selectable-session" key={thread.id}>
            {batch.selecting && <SessionCheckbox batch={batch} sessionKey={thread.id} title={title} />}
            <RenameableSession
              title={title}
              className={`thread-item ${selectedId === thread.id ? "selected" : ""}${running ? " running" : ""}`}
              disabled={batch.busy}
              selecting={batch.selecting}
              selected={selectedId === thread.id}
              renameBlockReason={thread.status === "Archived" ? sharedDisabledStates.renameArchivedThread : undefined}
              onSelect={() => batch.selecting ? batch.toggle(thread.id) : onSelect(thread.id)}
              onRename={(next) => rename.mutateAsync({ kind: "rename", id: thread.id, title: next })}
              menuActions={[
                { id: "archive", label: thread.status === "Archived" ? "恢复" : "归档", icon: thread.status === "Archived" ? <ArchiveRestore size={15} /> : <Archive size={15} />,
                  disabled: batch.busy, run: () => batch.prepareSingle(thread.id, thread.status === "Archived" ? "restore" : "archive") },
                { id: "copy-answer", label: "复制答复", icon: <Copy size={15} />, run: () => {
                  void getThread(thread.id).then(detail => copy(latestAssistantCopyText(detail.blocks))).catch(() => setMenuFeedback("读取答复失败，请重试"));
                } },
                { id: "copy-id", label: "复制 ID", icon: <Copy size={15} />, run: () => { void copy(thread.id); } },
                { id: "copy-path", label: "复制路径", icon: <Copy size={15} />, disabled: !thread.rollout_path, run: () => { void copy(thread.rollout_path); } },
                { id: "copy-resume", label: "复制恢复命令", icon: <Copy size={15} />, run: () => { void copy(threadResumeCommand(thread.id)); } },
                { id: "delete", label: "永久删除", icon: <Trash2 size={15} />, danger: true, disabled: batch.busy,
                  run: () => batch.prepareSingle(thread.id, "delete") },
              ]}
            >
              <span className="thread-item-content">
                <span className="thread-item-title">{title}</span>
                <span className="thread-item-meta">
                  {running ? (
                    <RunningIndicator />
                  ) : (
                    <span className={`thread-item-status ${thread.status}`}>{threadListItemStatusText(thread)}</span>
                  )}
                  {preview && <span className="thread-item-preview">{preview}</span>}
                  <SessionSize size={thread.storageSize} />
                </span>
              </span>
            </RenameableSession>
            </div>
          );
        })}
        {!loading && !error && threads.length === 0 && <div className="muted-row">没有匹配任务</div>}
      </div>
      {menuFeedback && <div role="status" className="task-feedback">{menuFeedback}</div>}
    </div>
  );
}
