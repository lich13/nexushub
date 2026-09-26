import { MessageSquare, RefreshCw, Search } from "lucide-react";
import { useState } from "react";
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

export const statusTabs = [
  { id: "all", label: "全部" },
  { id: "running", label: "运行中" },
  { id: "reply-needed", label: "待回复" },
  { id: "recoverable", label: "异常" },
  { id: "archived", label: "归档" }
];



export function ChatWorkspace({ csrfToken, mobileThreadsOpen, setMobileThreadsOpen, setView, capabilities }: {
  csrfToken?: string | null;
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
      csrfToken={csrfToken}
      onBatchSucceeded={(keys) => { for (const key of keys) threadCache.clearArchivedThreadClientState(messageStore, key); if (resolvedSelected && keys.includes(resolvedSelected)) selectThread(null); }}
    />
  );

  return (
    <div className={`chat-layout ${!mobileThreadsOpen && resolvedSelected ? "has-selection" : ""}`}>
      <aside className="thread-column">{list}</aside>
      <section className="conversation-column">
        {resolvedSelected && (selectedDetail || messageStore.getSlot(resolvedSelected).summary) ? (
          <Conversation
            threadId={resolvedSelected}
            detail={selectedDetail ?? threadDetailFromSlot(resolvedSelected, messageStore.getSlot(resolvedSelected), selectedThreadSummary)}
            slot={messageStore.getSlot(resolvedSelected)}
            messageStore={messageStore}
            csrfToken={csrfToken}
            onSelect={(id) => selectThread(id)}
            onPanelSelect={setView}
            nextThreadAfterArchive={nextThreadAfterRemoval}
            capabilities={capabilities}
            onBack={() => setMobileThreadsOpen(true)}
          />
        ) : (
          <div className="empty-state"><MessageSquare size={28} /><strong>{detailLoading ? "正在读取任务" : "选择一个任务"}</strong></div>
        )}
      </section>
    </div>
  );
}

function ThreadList({ status, q, setQ, setStatus, threads, selectedId, onSelect, onRefresh, loading, error, csrfToken, onBatchSucceeded }: {
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
  csrfToken?: string | null;
  onBatchSucceeded: (keys: string[]) => void;
}) {
  const batch = useSessionSelection("codex", `${status}\0${q}`, threads.map(t => t.id), csrfToken, onBatchSucceeded);
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
        <SessionBatchControls batch={batch} operations={status === "archived" ? ["restore", "delete"] : ["archive"]} />
        {error && <div className="form-error" role="alert">{error}</div>}
        {loading && <div className="muted-row">正在读取 Codex 状态...</div>}
        {threads.map((thread) => {
          const title = threadListItemText(thread);
          const preview = threadListItemPreviewText(thread);
          const running = isThreadListItemRunning(thread);
          return (
            <div className="selectable-session" key={thread.id}>
            {batch.selecting && <SessionCheckbox batch={batch} sessionKey={thread.id} title={title} />}
            <button className={`thread-item ${selectedId === thread.id ? "selected" : ""}${running ? " running" : ""}`} onClick={() => batch.selecting ? batch.toggle(thread.id) : onSelect(thread.id)} title={title} disabled={batch.busy}>
              <span className="thread-item-content">
                <span className="thread-item-title">{title}</span>
                <span className="thread-item-meta">
                  {running ? (
                    <RunningIndicator />
                  ) : (
                    <span className={`thread-item-status ${thread.status}`}>{threadListItemStatusText(thread)}</span>
                  )}
                  {preview && <span className="thread-item-preview">{preview}</span>}
                </span>
              </span>
            </button>
            </div>
          );
        })}
        {!loading && !error && threads.length === 0 && <div className="muted-row">没有匹配任务</div>}
      </div>
    </div>
  );
}
