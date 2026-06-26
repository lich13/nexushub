import { useEffect, useState } from "react";
import {
  combinedQuestionAnswers,
  currentPlanActionOptions,
  moveActionSelection,
  planActionSubmission,
  questionAnswerPayload,
  questionAnswersReady,
  selectionFromDigitKey
} from "../../lib/domain/conversationViewModel";
import type { MessageBlock, PendingElicitation } from "../../types";

export function CurrentActionCard({
  plan,
  pending,
  onAcceptPlan,
  onRevisePlan,
  planPending,
  onSubmitQuestion,
  questionPending,
  onDismiss
}: {
  plan?: MessageBlock | null;
  pending?: PendingElicitation | null;
  onAcceptPlan: (block: MessageBlock) => void;
  onRevisePlan: (block: MessageBlock, instructions: string) => void;
  planPending: boolean;
  onSubmitQuestion: (answers: Record<string, string[]>) => void;
  questionPending: boolean;
  onDismiss: () => void;
}) {
  const isPlan = Boolean(plan);
  const busy = isPlan ? planPending : questionPending;
  const questions = pending?.questions ?? [];
  const questionSignature = questions.map((question) => `${question.id}:${question.options.map((option) => option.label).join("|")}`).join(";");
  const [selected, setSelected] = useState(0);
  const [revision, setRevision] = useState("");
  const [questionAnswers, setQuestionAnswers] = useState<Record<string, string | string[] | undefined>>({});
  const [questionNotes, setQuestionNotes] = useState<Record<string, string>>({});
  const options = isPlan ? currentPlanActionOptions() : questions[0]?.options ?? [];
  const selectedPlanRequiresRevision = isPlan && selected === 1;
  const ready = isPlan
    ? Boolean(plan && planActionSubmission(selected, revision))
    : questionAnswersReady(questions, combinedQuestionAnswers(questions, questionAnswers, questionNotes));

  function submitAction() {
    if (busy || !ready) return;
    if (plan) {
      const submission = planActionSubmission(selected, revision);
      if (!submission) return;
      if (submission.action === "accept") {
        onAcceptPlan(plan);
      } else if (submission.action === "revise") {
        onRevisePlan(plan, submission.instructions);
      } else {
        onDismiss();
      }
      return;
    }
    if (pending) onSubmitQuestion(questionAnswerPayload(questions, combinedQuestionAnswers(questions, questionAnswers, questionNotes)));
  }

  useEffect(() => {
    setSelected(0);
    setRevision("");
    setQuestionAnswers((current) => {
      const initial: Record<string, string | string[] | undefined> = {};
      for (const question of questions) {
        initial[question.id] = current[question.id] ?? question.options[0]?.label;
      }
      return initial;
    });
    setQuestionNotes({});
  }, [plan?.id, pending?.turn_id, pending?.item_id, questionSignature]);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      const target = event.target as HTMLElement | null;
      const editable = target?.closest("input, textarea, select, [contenteditable='true']");
      if (event.key === "Escape") {
        event.preventDefault();
        onDismiss();
        return;
      }
      if (!editable && (event.key === "ArrowDown" || event.key === "ArrowUp")) {
        event.preventDefault();
        setSelected((current) => moveActionSelection(current, options.length, event.key === "ArrowDown" ? 1 : -1));
        return;
      }
      if (!editable) {
        const digitSelection = selectionFromDigitKey(event.key, options.length);
        if (digitSelection !== null) {
          event.preventDefault();
          setSelected(digitSelection);
          if (!isPlan && questions[0]?.options[digitSelection]) {
            setQuestionAnswers((current) => ({ ...current, [questions[0].id]: questions[0].options[digitSelection].label }));
          }
          return;
        }
      }
      if (event.key === "Enter" && !event.shiftKey) {
        event.preventDefault();
        submitAction();
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [busy, isPlan, onDismiss, options.length, questions, ready, revision, selected, questionAnswers, questionNotes]);

  const chooseQuestionOption = (questionId: string, label: string, index: number) => {
    setSelected(index);
    setQuestionAnswers((current) => ({ ...current, [questionId]: label }));
  };

  return (
    <section className="current-action-card" aria-live="polite">
      <div className="current-action-header">
        <div>
          <span>{isPlan ? "Plan Mode" : "Questions"}</span>
          <strong>{isPlan ? "实施此计划?" : questions[0]?.question ?? "Codex 正在等待选择"}</strong>
        </div>
        <small>↑↓ 选择 · 1-9 快选</small>
      </div>
      <div className="current-action-options">
        {isPlan ? options.map((option, index) => (
          <button
            type="button"
            key={option.label}
            className={selected === index ? "current-action-option selected" : "current-action-option"}
            onClick={() => setSelected(index)}
          >
            <span>{index + 1}</span>
            <div>
              <strong>{option.label}</strong>
              <small>{option.description}</small>
            </div>
          </button>
        )) : questions.map((question) => (
          <div className="current-action-question" key={question.id}>
            {questions.length > 1 && <strong>{question.question}</strong>}
            {question.options.map((option, index) => (
              <button
                type="button"
                key={`${question.id}-${option.label}`}
                className={questionAnswers[question.id] === option.label ? "current-action-option selected" : "current-action-option"}
                onClick={() => chooseQuestionOption(question.id, option.label, index)}
              >
                <span>{index + 1}</span>
                <div>
                  <strong>{option.label}</strong>
                  {option.description && <small>{option.description}</small>}
                </div>
              </button>
            ))}
            <textarea
              className="current-action-textarea"
              value={questionNotes[question.id] ?? ""}
              onChange={(event) => setQuestionNotes((current) => ({ ...current, [question.id]: event.target.value }))}
              placeholder="补充输入"
            />
          </div>
        ))}
      </div>
      {selectedPlanRequiresRevision && (
        <textarea
          className="current-action-textarea"
          value={revision}
          onChange={(event) => setRevision(event.target.value)}
          placeholder="告诉 Codex 需要怎样调整计划"
        />
      )}
      <div className="current-action-footer">
        <button className="secondary-button ghost" type="button" onClick={onDismiss}>
          忽略 <kbd>ESC</kbd>
        </button>
        <button className="primary-button" type="button" disabled={!ready || busy} onClick={submitAction}>
          提交 <kbd>↵</kbd>
        </button>
      </div>
    </section>
  );
}
