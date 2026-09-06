import { ChevronRight } from "lucide-react";
import { useState } from "react";
import { MarkdownContent } from "../common/MarkdownContent";
import {
  blockKindLabel,
  conversationMessagePresentation,
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
  onShowHistory,
  historyExpanded = false
}: {
  block: MessageBlock;
  onShowHistory?: () => void;
  historyExpanded?: boolean;
}) {
  if (isHistoryCollapsedBlock(block)) {
    return <HistoryCollapseCell block={block} onShowHistory={onShowHistory} expanded={historyExpanded} />;
  }
  if (isPlanBlock(block)) {
    return <ProposedPlanCell block={block} />;
  }
  if (isQuestionBlock(block)) {
    return <QuestionCell block={block} />;
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
        <MarkdownContent text={messageBlockText(block)} />
      </div>
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

function ProposedPlanCell({ block }: { block: MessageBlock }) {
  return (
    <article className="plan-cell">
      <div className="message-meta">
        <span>Proposed Plan</span>
        <small>{block.plan_status || block.status || block.turn_id || block.item_id || block.kind}</small>
      </div>
      <div className="plan-body"><MarkdownContent text={extractPlanText(block.text || "")} /></div>
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

function QuestionCell({ block }: { block: MessageBlock }) {
  return (
    <article className="question-cell active-choice">
      <div className="message-meta">
        <span>Questions</span>
        <small>{block.turn_id || block.item_id || block.call_id || "request_user_input"}</small>
      </div>
      {block.questions.map((question) => (
        <div key={question.id} className="question-block">
          <strong>{question.question}</strong>
          <div className="choice-grid readonly-choices">
            {question.options.map((option, index) => (
              <div
                key={`${question.id}-${option.label}`}
                className="choice-option"
              >
                <span>{index + 1}</span>
                <strong>{option.label}</strong>
                {option.description && <small>{option.description}</small>}
              </div>
            ))}
          </div>
        </div>
      ))}
    </article>
  );
}
