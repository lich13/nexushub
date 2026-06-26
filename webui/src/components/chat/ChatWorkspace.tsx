import { Plus, RefreshCw, Search, X } from "lucide-react";
import { useState } from "react";
import { threadDetailFromSlot, useConversationController } from "../../hooks/useConversationController";
import {
  codexLocalCopy,
  isThreadListItemRunning,
  threadListItemPreviewText,
  threadListItemStatusText,
  threadListItemText,
  type SelectedThread,
  type View
} from "../../lib/domain/codexViewModel";
import type { RuntimeCapabilityMatrix } from "../../lib/query/system";
import type { ThreadSummary } from "../../types";
import { Conversation, EmptyConversation } from "./Conversation";

export const statusTabs = [
  { id: "all", label: "全部" },
  { id: "running", label: "运行中" },
  { id: "reply-needed", label: "待回复" },
  { id: "recoverable", label: "异常" }
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
      onNew={() => selectThread("__new")}
      onRefresh={() => threadCache.invalidateThreads()}
      loading={threads.isLoading}
    />
  );

  return (
    <div className="chat-layout">
      <aside className="thread-column desktop-only">{list}</aside>
      {mobileThreadsOpen && (
        <div className="drawer-backdrop" onClick={() => setMobileThreadsOpen(false)}>
          <aside className="thread-drawer" onClick={(event) => event.stopPropagation()}>
            <button className="icon-button drawer-close" onClick={() => setMobileThreadsOpen(false)} title="关闭"><X size={18} /></button>
            {list}
          </aside>
        </div>
      )}
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
          />
        ) : (
          <EmptyConversation
            loading={Boolean(resolvedSelected && detailLoading)}
            csrfToken={csrfToken}
            onCreated={(id) => selectThread(id)}
            onPanelSelect={setView}
            capabilities={capabilities}
          />
        )}
      </section>
    </div>
  );
}

function ThreadList({ status, q, setQ, setStatus, threads, selectedId, onSelect, onNew, onRefresh, loading }: {
  status: string;
  q: string;
  setQ: (value: string) => void;
  setStatus: (value: string) => void;
  threads: ThreadSummary[];
  selectedId: string | null;
  onSelect: (id: SelectedThread) => void;
  onNew: () => void;
  onRefresh: () => void;
  loading: boolean;
}) {
  return (
    <div className="thread-list">
      <div className="section-title thread-title-row">
        <div>
          <span>{codexLocalCopy.threadListEyebrow}</span>
          <strong>线程</strong>
        </div>
        <div className="thread-title-actions">
          <button className="icon-button compact" onClick={onRefresh} title="刷新线程"><RefreshCw size={16} /></button>
          <button className="icon-button compact primary-icon" onClick={onNew} title="新建线程"><Plus size={16} /></button>
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
        {loading && <div className="muted-row">正在读取 Codex 状态...</div>}
        {threads.map((thread) => {
          const title = threadListItemText(thread);
          const preview = threadListItemPreviewText(thread);
          const running = isThreadListItemRunning(thread);
          return (
            <button key={thread.id} className={`thread-item ${selectedId === thread.id ? "selected" : ""}${running ? " running" : ""}`} onClick={() => onSelect(thread.id)} title={title}>
              <span className="thread-item-content">
                <span className="thread-item-title">{title}</span>
                <span className="thread-item-meta">
                  {running ? (
                    <span className="thread-running-indicator" aria-label="运行中" title="运行中">
                      <span className="thread-running-spinner" aria-hidden="true" />
                    </span>
                  ) : (
                    <span className={`thread-item-status ${thread.status}`}>{threadListItemStatusText(thread)}</span>
                  )}
                  {preview && <span className="thread-item-preview">{preview}</span>}
                </span>
              </span>
            </button>
          );
        })}
        {!loading && threads.length === 0 && <div className="muted-row">没有匹配线程</div>}
      </div>
    </div>
  );
}


