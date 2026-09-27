import { File, ImageOff, X } from "lucide-react";
import { createContext, useContext, useEffect, useRef, useState } from "react";
import type { SessionProvider } from "../../lib/api/sessions";
import type { UserAttachment, UserMessageContent } from "../../types";
import { readAttachment } from "../../lib/query/attachments";
import { visibleMarkdown } from "../../lib/domain/visibleMarkdown";
import { FilePathLink } from "./FilePathLink";

export const UserMessageScope = createContext<{ provider: SessionProvider; sessionKey: string } | null>(null);

export function UserMessage({ message, text = "" }: { message?: UserMessageContent | null; text?: string }) {
  const body = visibleMarkdown(message?.text ?? text);
  if (!body.trim() && !message?.attachments.length) return null;
  return <article className="user-message">
    {!!message?.attachments.length && <div className="user-attachments" aria-label="消息附件">{message.attachments.map(attachment => <Attachment key={attachment.id} messageId={message.id} attachment={attachment} />)}</div>}
    {!!body.trim() && <div className="user-message-bubble">{body}</div>}
  </article>;
}

function Attachment({ attachment, messageId }: { attachment: UserAttachment; messageId: string }) {
  const scope = useContext(UserMessageScope);
  const container = useRef<HTMLDivElement>(null);
  const trigger = useRef<HTMLButtonElement>(null);
  const dialog = useRef<HTMLDialogElement>(null);
  const [visible, setVisible] = useState(false);
  const [url, setUrl] = useState("");
  const [error, setError] = useState("");
  const [preview, setPreview] = useState(false);
  const [attempt, setAttempt] = useState(0);
  const { provider, sessionKey } = scope ?? {};
  useEffect(() => {
    if (!container.current || attachment.kind !== "image" || attachment.reason) return;
    if (typeof IntersectionObserver === "undefined") { setVisible(true); return; }
    const observer = new IntersectionObserver(entries => setVisible(entries.some(entry => entry.isIntersecting)), { rootMargin: "240px" });
    observer.observe(container.current);
    return () => observer.disconnect();
  }, [attachment.id, attachment.kind, attachment.reason]);
  useEffect(() => {
    if ((!visible && !preview) || !provider || !sessionKey || attachment.reason || attachment.kind !== "image") { setUrl(""); return; }
    let cancelled = false;
    setError("");
    readAttachment({ provider, sessionKey, messageId, attachmentId: attachment.id }).then(data => {
      if (!cancelled) setUrl(data);
    }, reason => { if (!cancelled) setError(reason instanceof Error ? reason.message : "图片暂时无法读取"); });
    return () => { cancelled = true; };
  }, [visible, preview, provider, sessionKey, messageId, attachment.id, attachment.kind, attachment.reason, attempt]);
  useEffect(() => {
    if (preview) dialog.current?.showModal();
    else if (dialog.current?.open) dialog.current.close();
  }, [preview]);
  const reason = attachment.reason || error;
  const image = attachment.kind === "image";
  return <div ref={container} className={`user-attachment ${image ? "image-attachment" : "file-attachment"}`}>
    {image ? <button ref={trigger} type="button" className="attachment-thumbnail" aria-label={`预览图片：${attachment.name}`} title={reason || attachment.name} disabled={!url || !!reason} onClick={() => setPreview(true)}>
      {url && !reason ? <img src={url} alt={attachment.name} onError={() => setError("图片内容损坏，无法预览")} /> : <><ImageOff size={23} /><span>{reason ? "图片不可用" : "图片"}</span></>}
    </button> : <File size={18} />}
    {(!image || reason) && <div className="attachment-description">
      {attachment.path ? <FilePathLink href={attachment.path}>{attachment.name}</FilePathLink> : <span>{attachment.name}</span>}
      {reason && <small>{reason}</small>}
      {error && !attachment.reason && <button type="button" className="file-path-label" onClick={() => setAttempt(value => value + 1)}>重试预览</button>}
    </div>}
    {image && <dialog ref={dialog} className="attachment-preview" aria-label={`图片预览：${attachment.name}`} onClose={() => { setPreview(false); trigger.current?.focus(); }} onClick={event => { if (event.target === event.currentTarget) setPreview(false); }}>
      <header><span>{attachment.name}</span><button type="button" className="icon-button" aria-label="关闭图片预览" onClick={() => setPreview(false)} autoFocus><X size={20} /></button></header>
      {preview && url && <img src={url} alt={attachment.name} />}
    </dialog>}
  </div>;
}
