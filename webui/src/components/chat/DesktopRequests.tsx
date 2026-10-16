import { useState } from "react";
import type { DesktopAction, DesktopPendingRequest, DesktopQuestion } from "../../lib/query/desktop";

type Action = (action: DesktopAction) => Promise<void>;
// Native question/property IDs are data, including names such as __proto__.
const ownValue = (values: Record<string, string>, key: string) => Object.prototype.hasOwnProperty.call(values, key) ? values[key] : undefined;

export function DesktopRequests({ requests, busy, act, open }: { requests: DesktopPendingRequest[]; busy: boolean; act: Action; open: () => Promise<void> }) {
  const [expanded, setExpanded] = useState<string | null>(null);
  return <div className="desktop-requests">{requests.map((request, index) => {
    const shown = expanded === request.key || (expanded === null && index === 0);
    return <section key={request.key} className="desktop-request">
      <button className="desktop-request-title" aria-expanded={shown} onClick={() => setExpanded(shown ? "" : request.key)}>{request.title}</button>
      {shown && <RequestBody key={request.key} request={request} busy={busy} act={act} open={open} />}
    </section>;
  })}</div>;
}

function RequestBody({ request, busy, act, open }: { request: DesktopPendingRequest; busy: boolean; act: Action; open: () => Promise<void> }) {
  const run = (action: DesktopAction) => { void act(action).catch(() => {}); };
  return <div className="desktop-request-body">
    {request.body && <pre>{request.body}</pre>}
    {request.canAccept && request.kind === "question" ? <Questions request={request} busy={busy} act={act} />
      : request.canAccept && request.kind === "elicitation" ? <Elicitation request={request} busy={busy} act={act} />
        : <div className="desktop-request-actions">
          {["command", "file", "permissions"].includes(request.kind) && <>
            <button disabled={busy} onClick={() => run({ type: "decide", requestKey: request.key, allow: false })}>拒绝</button>
            {request.canAccept && <button disabled={busy} onClick={() => run({ type: "decide", requestKey: request.key, allow: true })}>允许本次</button>}
          </>}
          {request.kind === "elicitation" && <button disabled={busy} onClick={() => run({ type: "elicitation", requestKey: request.key, accept: false, content: null })}>拒绝</button>}
          {!request.canAccept && <span className="muted-text">请在 Codex 中查看完整请求并处理。</span>}
          <button disabled={busy} onClick={() => { void open(); }}>在 Codex 中打开</button>
        </div>}
  </div>;
}

function Questions({ request, busy, act }: { request: DesktopPendingRequest; busy: boolean; act: Action }) {
  const [answers, setAnswers] = useState<Record<string, string>>({});
  const [choices, setChoices] = useState<Record<string, string>>({});
  const answer = (question: DesktopQuestion) => ownValue(answers, question.id)?.trim() ? ownValue(answers, question.id)! : ownValue(choices, question.id) ?? "";
  return <form onSubmit={event => {
    event.preventDefault();
    void act({ type: "answer", requestKey: request.key, answers: request.questions.map(question => ({ questionId: question.id, answers: [answer(question)] })) }).catch(() => {});
  }}>
    {request.questions.map(question => <fieldset key={question.id} disabled={busy}>
      <legend>{question.header || question.question}</legend>
      {question.header && <p>{question.question}</p>}
      {question.options.map((option, index) => <label className="desktop-answer-choice" key={index}>
        <input type="radio" name={`${request.key}:${question.id}`} checked={ownValue(choices, question.id) === option.label && !ownValue(answers, question.id)} onChange={() => { setChoices(value => ({ ...value, [question.id]: option.label })); setAnswers(value => ({ ...value, [question.id]: "" })); }} />
        <span>{option.label}{option.description && <small>{option.description}</small>}</span>
      </label>)}
      <label className="desktop-free-answer"><span>{question.options.length ? "或填写回答" : "回答"}</span>
        <input type={question.secret ? "password" : "text"} autoComplete="off" maxLength={4096} value={ownValue(answers, question.id) ?? ""} onChange={event => setAnswers(value => ({ ...value, [question.id]: event.target.value }))} />
      </label>
    </fieldset>)}
    <button disabled={busy || request.questions.some(question => !answer(question).trim())}>提交回答</button>
  </form>;
}

type PrimitiveSchema = { type: string; title?: string; description?: string; enum?: (string | number | boolean)[]; enumNames?: string[]; minLength?: number; maxLength?: number; minimum?: number; maximum?: number };
function Elicitation({ request, busy, act }: { request: DesktopPendingRequest; busy: boolean; act: Action }) {
  const schema = request.formSchema as { properties: Record<string, PrimitiveSchema>; required?: string[] };
  const [values, setValues] = useState<Record<string, string>>({});
  const [error, setError] = useState("");
  const properties = Object.entries(schema.properties);
  return <form onSubmit={event => {
    event.preventDefault(); setError("");
    try {
      const content = Object.fromEntries(properties.filter(([key]) => ownValue(values, key) !== undefined && ownValue(values, key) !== "").map(([key, field]) => {
        const value = ownValue(values, key)!;
        if (field.enum) return [key, JSON.parse(value) as unknown];
        if (field.type === "boolean") return [key, value === "true"];
        if (field.type === "number" || field.type === "integer") { const number = Number(value); if (!Number.isFinite(number)) throw new Error(); return [key, number]; }
        return [key, value];
      }));
      void act({ type: "elicitation", requestKey: request.key, accept: true, content }).catch(() => {});
    } catch { setError("请检查表单内容"); }
  }}>
    {properties.map(([key, field]) => <label key={key} className="desktop-free-answer"><span>{field.title || key}{schema.required?.includes(key) ? " *" : ""}</span>
      {field.enum || field.type === "boolean" ? <select disabled={busy} required={schema.required?.includes(key)} value={ownValue(values, key) ?? ""} onChange={event => setValues(value => ({ ...value, [key]: event.target.value }))}>
        <option value="">请选择</option>{(field.enum ?? [true, false]).map((value, index) => <option key={index} value={JSON.stringify(value)}>{field.enumNames?.[index] ?? (typeof value === "boolean" ? value ? "是" : "否" : String(value))}</option>)}
      </select> : <input disabled={busy} autoComplete="off" required={schema.required?.includes(key)} type={["number", "integer"].includes(field.type) ? "number" : "text"} step={field.type === "integer" ? 1 : "any"} min={field.minimum} max={field.maximum} minLength={field.minLength} maxLength={Math.min(field.maxLength ?? 4096, 4096)} value={ownValue(values, key) ?? ""} onChange={event => setValues(value => ({ ...value, [key]: event.target.value }))} />}
      {field.description && <small>{field.description}</small>}
    </label>)}
    {error && <p role="alert">{error}</p>}
    <div className="desktop-request-actions"><button type="button" disabled={busy} onClick={() => { void act({ type: "elicitation", requestKey: request.key, accept: false, content: null }).catch(() => {}); }}>拒绝</button><button disabled={busy}>提交</button></div>
  </form>;
}
