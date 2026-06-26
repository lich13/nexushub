import { ChevronRight, ClipboardCheck, X } from "lucide-react";
import { useMemo, useState } from "react";
import {
  blockKindLabel,
  conversationMessagePresentation,
  formatPayload,
  formatTime,
  historyCollapseKind,
  isHistoryCollapsedBlock,
  isPlanBlock,
  isQuestionBlock,
  isQuestionResultBlock,
  isRunningToolBlock,
  isToolBlock,
  messageBlockText,
  roleLabel,
  segmentInternalReferences,
  shouldRenderConversationMessage,
  toolBlockDetailText,
  toolBlockStatus,
  toolBlockSummary,
  toolBlockTitle
} from "../../lib/domain/conversationViewModel";
import { extractPlanText } from "../../lib/domain/codexViewModel";
import type { MessageBlock } from "../../types";

export function MessageBlockView({
  block,
  activePlan = false,
  planPending = false,
  activeQuestion = false,
  questionPending = false,
  onShowHistory,
  historyExpanded = false
}: {
  block: MessageBlock;
  activePlan?: boolean;
  planPending?: boolean;
  activeQuestion?: boolean;
  questionPending?: boolean;
  onShowHistory?: () => void;
  historyExpanded?: boolean;
}) {
  if (isHistoryCollapsedBlock(block)) {
    return <HistoryCollapseCell block={block} onShowHistory={onShowHistory} expanded={historyExpanded} />;
  }
  if (isPlanBlock(block)) {
    return (
      <ProposedPlanCell
        block={block}
        active={activePlan}
        pending={planPending}
      />
    );
  }
  if (isQuestionBlock(block)) {
    if (activeQuestion) return <QuestionCell block={block} pendingSubmit={questionPending} />;
    return <QuestionResultCell block={block} />;
  }
  if (isQuestionResultBlock(block)) {
    return <QuestionResultCell block={block} />;
  }
  if (isToolBlock(block)) {
    return <ToolBlockView block={block} />;
  }
  if (!shouldRenderConversationMessage(block)) {
    return null;
  }
  const presentation = conversationMessagePresentation(block);
  return (
    <article className={presentation.rowClassName}>
      <div className="chat-meta">
        <span>{roleLabel(block.role)}</span>
        <small>{blockKindLabel(block.kind)}{block.created_at ? ` · ${formatTime(block.created_at)}` : ""}</small>
      </div>
      <div className={presentation.bodyClassName}>
        <MessageContent text={messageBlockText(block)} />
      </div>
    </article>
  );
}

export function ApprovalCard({ block, onDecision, pending }: { block: MessageBlock; onDecision: (decision: string) => void; pending: boolean }) {
  return (
    <article className="approval-card action-request">
      <div className="message-meta">
        <span>审批请求</span>
        <small>{block.call_id || block.item_id || block.turn_id || block.kind}</small>
      </div>
      <pre>{block.text || formatPayload(block.payload) || "Codex 正在等待权限审批。"}</pre>
      <div className="button-row">
        <button className="primary-button" disabled={pending} onClick={() => onDecision("accept")}>
          <ClipboardCheck size={17} />批准
        </button>
        <button className="danger-button soft" disabled={pending} onClick={() => onDecision("decline")}>
          <X size={17} />拒绝
        </button>
      </div>
    </article>
  );
}

export function UnsupportedApprovalCard({ block }: { block: MessageBlock }) {
  return (
    <article className="approval-card action-request">
      <div className="message-meta">
        <span>审批请求</span>
        <small>{block.call_id || block.item_id || block.turn_id || block.kind}</small>
      </div>
      <pre>{block.text || formatPayload(block.payload) || "Codex 正在等待权限审批。"}</pre>
      <div className="muted-row">macOS App 当前不支持在此面板处理权限审批，请在 Codex 原生会话中处理。</div>
    </article>
  );
}

function ToolBlockView({ block }: { block: MessageBlock }) {
  const [open, setOpen] = useState(false);
  const summary = toolBlockSummary(block);
  return (
    <details
      className={`tool-card ${isRunningToolBlock(block) ? "running" : ""}`}
      onToggle={(event) => setOpen((event.currentTarget as HTMLDetailsElement).open)}
    >
      <summary>
        <span className="tool-title">{toolBlockTitle(block)}</span>
        <small>{toolBlockStatus(block)}</small>
        <ChevronRight size={16} />
      </summary>
      {summary && <div className="tool-summary">{summary}</div>}
      {open && <pre>{toolBlockDetailText(block)}</pre>}
    </details>
  );
}

function MessageContent({ text }: { text: string }) {
  const [copied, setCopied] = useState<string | null>(null);
  const segments = useMemo(() => segmentInternalReferences(text), [text]);
  return (
    <>
      {segments.map((segment, index) => {
        if (segment.type === "text") {
          return <span key={`text-${index}`}>{segment.text}</span>;
        }
        return (
          <button
            key={`ref-${index}-${segment.text}`}
            type="button"
            className="internal-reference"
            title="复制内部引用"
            onClick={async () => {
              const copyText = segment.copyText ?? segment.text;
              await navigator.clipboard?.writeText(copyText);
              setCopied(copyText);
              window.setTimeout(() => setCopied((current) => current === copyText ? null : current), 1600);
            }}
          >
            {segment.text}
            {copied === (segment.copyText ?? segment.text) && <small>已复制</small>}
          </button>
        );
      })}
    </>
  );
}

function HistoryCollapseCell({ block, onShowHistory, expanded }: { block: MessageBlock; onShowHistory?: () => void; expanded: boolean }) {
  const kind = historyCollapseKind(block);
  const label = toolBlockSummary(block) ?? (kind === "tool" ? "历史工具活动已折叠" : kind === "action" ? "历史计划和问题已折叠" : "较早消息已折叠");
  const eyebrow = kind === "tool" ? "Tool activity" : kind === "action" ? "Plan & questions" : "Earlier messages";
  return (
    <article className="history-collapse-cell">
      <div>
        <span>{eyebrow}</span>
        <strong>{label}</strong>
      </div>
      {onShowHistory && (
        <button className="secondary-button" disabled={expanded} onClick={onShowHistory} type="button">
          {expanded ? "已显示全部" : "显示全部历史"}
        </button>
      )}
    </article>
  );
}

function ProposedPlanCell({ block, active, pending }: { block: MessageBlock; active: boolean; pending: boolean }) {
  return (
    <article className={active ? "plan-cell active" : "plan-cell"}>
      <div className="message-meta">
        <span>Proposed Plan</span>
        <small>{block.plan_status || block.status || block.turn_id || block.item_id || block.kind}</small>
      </div>
      <div className="plan-body">{extractPlanText(block.text || "")}</div>
      {active && pending && <div className="action-inline-status">正在提交计划操作...</div>}
    </article>
  );
}

function QuestionResultCell({ block }: { block: MessageBlock }) {
  const answers = block.answers ?? [];
  return (
    <article className="question-result-cell">
      <div className="message-meta">
        <span>Questions</span>
        <small>{block.status || "completed"}</small>
      </div>
      {answers.length > 0 ? (
        <div className="answered-list">
          {answers.map((answer) => (
            <div className="answered-row" key={answer.question_id}>
              <span>{answer.question_id}</span>
              <strong>{answer.answers.length ? answer.answers.join(", ") : "未回答"}</strong>
              {answer.note && <small>{answer.note}</small>}
            </div>
          ))}
        </div>
      ) : (
        <p>Questions answered</p>
      )}
    </article>
  );
}

function QuestionCell({ block, pendingSubmit }: { block: MessageBlock; pendingSubmit: boolean }) {
  return (
    <article className="question-cell active-choice">
      <div className="message-meta">
        <span>Questions</span>
        <small>{block.turn_id || block.item_id || block.call_id || "request_user_input"}</small>
      </div>
      {block.questions.map((question) => (
        <div key={question.id} className="question-block">
          <strong>{question.question}</strong>
          <div className="choice-grid">
            {question.options.map((option, index) => (
              <button
                key={`${question.id}-${option.label}`}
                className="choice-option"
                disabled
                type="button"
              >
                <span>{index + 1}</span>
                <strong>{option.label}</strong>
                {option.description && <small>{option.description}</small>}
              </button>
            ))}
          </div>
        </div>
      ))}
      {pendingSubmit && <div className="action-inline-status">正在提交选择...</div>}
    </article>
  );
}
