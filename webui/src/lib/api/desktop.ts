import { connectionSnapshot, createNativeChannel, invokeNative, runtimeRpc } from "../runtime";

export type DesktopMode = "default" | "plan";
export type DesktopThreadRequest = { revision: number; threadId: string };
export type DesktopSessionRequest = { revision: number; sessionId: string };
export type DesktopAnswer = { questionId: string; answers: string[] };
export type DesktopReferenceSpan = { handle: string; start: number; end: number };
export type DesktopAction =
  | { type: "send"; messageId: string; text: string; references: string[]; referenceSpans?: DesktopReferenceSpan[] }
  | { type: "steer"; messageId: string; text: string; references: string[]; referenceSpans?: DesktopReferenceSpan[]; turnId: string }
  | { type: "stop"; turnId: string }
  | { type: "cancelQueued" | "dismissUnknown"; messageId: string }
  | { type: "resumeQueue" }
  | { type: "settings"; model: string; reasoningEffort: string | null; mode: DesktopMode }
  | { type: "decide"; requestKey: string; allow: boolean }
  | { type: "answer"; requestKey: string; answers: DesktopAnswer[] }
  | { type: "elicitation"; requestKey: string; accept: boolean; content: unknown | null };
export type DesktopActionRequest = DesktopSessionRequest & { action: DesktopAction };
export type DesktopQueuedMessage = { id: string; status: string; kind: string; preview: string; createdAt: number };
export type DesktopQuestionOption = { label: string; description: string };
export type DesktopQuestion = { id: string; header: string; question: string; secret: boolean; options: DesktopQuestionOption[] };
export type DesktopPendingRequest = { key: string; kind: string; title: string; body: string; questions: DesktopQuestion[]; formSchema: unknown | null; canAccept: boolean };
export type DesktopView = {
  sessionId: string; threadId: string; revision: number; connected: boolean; writable: boolean;
  runtime: string; activeTurnId: string | null; model: string | null; reasoningEffort: string | null;
  mode: DesktopMode | null; requests: DesktopPendingRequest[]; queue: DesktopQueuedMessage[]; error: string | null;
};
export type DesktopCatalogRequest = { revision: number; sessionId: string | null; projectHandle: string | null; kind: string; query: string; cursor: string | null };
export type DesktopCatalogItem = { handle: string; kind: string; name: string; description: string; reasoningEfforts: string[]; defaultReasoningEffort: string | null };
export type DesktopCatalog = { items: DesktopCatalogItem[]; nextCursor: string | null };
export type DesktopChooseRequest = { revision: number; sessionId: string | null; kind: string };
export type DesktopChosenItem = { handle: string; name: string; kind: string };
export type DesktopChoice = { item: DesktopChosenItem | null };
export type DesktopCreateRequest = { revision: number; projectHandle: string };
export type DesktopCreatedThread = { threadId: string };
export type DesktopAck = { ok: boolean };

function localRevision(): number {
  const connection = connectionSnapshot();
  if (connection.target !== "local" || connection.changing) throw new Error("桌面协作仅支持本机 Codex");
  return connection.revision;
}

export async function attachDesktopThread(threadId: string, receive: (view: DesktopView) => void) {
  const revision = localRevision();
  let latest: DesktopView | undefined;
  const channel = createNativeChannel<DesktopView>(view => {
    if (connectionSnapshot().revision !== revision || view.threadId !== threadId) return;
    latest = view; receive(view);
  });
  try {
    const initial = await runtimeRpc<DesktopView>("codex.desktop.attach", { request: { revision, threadId } satisfies DesktopThreadRequest, onEvent: channel.value }, true);
    return { initial: latest ?? initial, detach: async () => {
      channel.close();
      await detachDesktopThread({ revision, sessionId: initial.sessionId });
    } };
  } catch (error) { channel.close(); throw error; }
}

export function detachDesktopThread(request: DesktopSessionRequest) {
  return invokeNative<DesktopAck>("codex.desktop.detach", { request });
}
export function actOnDesktopThread(sessionId: string, action: DesktopAction) {
  return runtimeRpc<DesktopView>("codex.desktop.action", { request: { revision: localRevision(), sessionId, action } satisfies DesktopActionRequest }, true);
}
export function openDesktopThread(threadId: string) {
  return runtimeRpc<DesktopAck>("codex.desktop.open", { request: { revision: localRevision(), threadId } satisfies DesktopThreadRequest }, true);
}
export function desktopCatalog(request: Omit<DesktopCatalogRequest, "revision">) {
  return runtimeRpc<DesktopCatalog>("codex.desktop.catalog", { request: { ...request, revision: localRevision() } satisfies DesktopCatalogRequest }, true);
}
export function chooseDesktopItem(kind: string, sessionId: string | null = null) {
  return runtimeRpc<DesktopChoice>("codex.desktop.choose", { request: { revision: localRevision(), sessionId, kind } satisfies DesktopChooseRequest }, true);
}
export function createDesktopThread(projectHandle: string) {
  return runtimeRpc<DesktopCreatedThread>("codex.desktop.create", { request: { revision: localRevision(), projectHandle } satisfies DesktopCreateRequest }, true);
}
