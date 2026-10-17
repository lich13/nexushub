import { connectionSnapshot, createNativeChannel, invokeNative, runtimeRpc } from "../runtime";

export type DesktopMode = "default" | "plan";
export type DesktopDiagnosticsRequest = { revision: number };
export type DesktopDiagnostic = { timestamp: number; source: string; method: string; version: number | null; code: number | null; category: string; hint: string };
export type DesktopDiagnostics = { records: DesktopDiagnostic[]; desktopBuild: string | null; bundledCliVersion: string | null };
export type DesktopThreadRequest = { confirmOwner?: boolean; revision: number; threadId: string };
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
  | { type: "decide"; requestKey: string; allow: boolean; decision?: string }
  | { type: "answer"; requestKey: string; answers: DesktopAnswer[] }
  | { type: "elicitation"; requestKey: string; accept: boolean; content: unknown | null };
export type DesktopActionRequest = DesktopSessionRequest & { action: DesktopAction };
export type DesktopQueuedMessage = { id: string; status: string; kind: string; preview: string; createdAt: number };
export type DesktopQuestionOption = { label: string; description: string };
export type DesktopQuestion = { id: string; header: string; question: string; secret: boolean; options: DesktopQuestionOption[] };
export type DesktopFileReview = { path: string; kind: string; diff: string; truncated: boolean };
export type DesktopPermissionsReview = { network: string | null; fileSystem: string | null };
export type DesktopReview = { command: string | null; cwd: string | null; reason: string | null; changes: DesktopFileReview[]; permissions: DesktopPermissionsReview | null; extra: string | null; truncated: boolean };
export type DesktopPendingRequest = { key: string; kind: string; title: string; body: string; questions: DesktopQuestion[]; formSchema: unknown | null; canAccept: boolean; supportedDecisions?: string[]; review?: DesktopReview | null };
export type DesktopView = {
  connectionPhase?: string | null; desktopBuild?: string | null;
  sessionId: string; threadId: string; revision: number; connected: boolean; writable: boolean;
  runtime: string; activeTurnId: string | null; model: string | null; reasoningEffort: string | null;
  mode: DesktopMode | null; requests: DesktopPendingRequest[]; queue: DesktopQueuedMessage[]; error: string | null;
};
export type DesktopCatalogRequest = { forceRefetch?: boolean; revision: number; sessionId: string | null; projectHandle: string | null; kind: string; query: string; cursor: string | null };
export type DesktopCatalogItem = { handle: string; kind: string; name: string; description: string; reasoningEfforts: string[]; defaultReasoningEffort: string | null };
export type DesktopCatalog = { items: DesktopCatalogItem[]; nextCursor: string | null };
export type DesktopChooseRequest = { revision: number; sessionId: string | null; kind: string };
export type DesktopChosenItem = { handle: string; name: string; kind: string };
export type DesktopChoice = { item: DesktopChosenItem | null };
export type DesktopAttachmentBounds = { x: number; y: number; width: number; height: number };
export type DesktopAttachmentAction =
  | { type: "bind"; bounds: DesktopAttachmentBounds; handles: string[] }
  | { type: "bounds"; bindingId: string; bounds: DesktopAttachmentBounds; handles: string[] }
  | { type: "paste"; bindingId: string }
  | { type: "release"; bindingId: string; handles: string[] }
  | { type: "unbind"; bindingId: string };
export type DesktopAttachmentRequest = DesktopSessionRequest & { action: DesktopAttachmentAction };
export type DesktopAttachmentEvent = { bindingId: string; items: DesktopChosenItem[]; error: string | null };
export type DesktopAttachmentResult = { bindingId: string };
export type DesktopCreateRequest = { revision: number; projectHandle: string };
export type DesktopCreatedThread = { threadId: string };
export type DesktopAck = { ok: boolean };

function localRevision(): number {
  const connection = connectionSnapshot();
  if (connection.target !== "local" || connection.changing) throw new Error("桌面协作仅支持本机 Codex");
  return connection.revision;
}

export async function attachDesktopThread(threadId: string, receive: (view: DesktopView) => void, confirmOwner = true) {
  const revision = localRevision();
  let latest: DesktopView | undefined;
  const channel = createNativeChannel<DesktopView>(view => {
    if (connectionSnapshot().revision !== revision || view.threadId !== threadId) return;
    latest = view; receive(view);
  });
  try {
    const initial = await runtimeRpc<DesktopView>("codex.desktop.attach", { request: { revision, threadId, confirmOwner } satisfies DesktopThreadRequest, onEvent: channel.value }, true);
    return { initial: latest ?? initial, detach: async () => {
      channel.close();
      await detachDesktopThread({ revision, sessionId: latest?.sessionId ?? initial.sessionId });
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
export function desktopDiagnostics() {
  return runtimeRpc<DesktopDiagnostics>("codex.desktop.diagnostics", { request: { revision: localRevision() } satisfies DesktopDiagnosticsRequest }, true);
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

export async function bindDesktopAttachments(sessionId: string, bounds: DesktopAttachmentBounds, handles: string[], receive: (event: DesktopAttachmentEvent) => void) {
  const revision = localRevision();
  let bindingId: string | undefined;
  let closed = false;
  const pending: DesktopAttachmentEvent[] = [];
  const channel = createNativeChannel<DesktopAttachmentEvent>(event => {
    if (closed || connectionSnapshot().revision !== revision) return;
    if (!bindingId) { if (pending.length < 32) pending.push(event); return; }
    if (event.bindingId === bindingId) receive(event);
  });
  const action = (action: DesktopAttachmentAction) => runtimeRpc<DesktopAttachmentResult>("codex.desktop.attachments", { request: { revision, sessionId, action } satisfies DesktopAttachmentRequest, onEvent: channel.value }, true);
  try {
    const result = await runtimeRpc<DesktopAttachmentResult>("codex.desktop.attachments", { request: { revision, sessionId, action: { type: "bind", bounds, handles } } satisfies DesktopAttachmentRequest, onEvent: channel.value }, true);
    bindingId = result.bindingId;
    for (const event of pending) if (event.bindingId === bindingId && connectionSnapshot().revision === revision) receive(event);
    return {
      update: (bounds: DesktopAttachmentBounds, handles: string[]) => action({ type: "bounds", bindingId: result.bindingId, bounds, handles }),
      paste: () => action({ type: "paste", bindingId: result.bindingId }),
      release: (handles: string[]) => action({ type: "release", bindingId: result.bindingId, handles }),
      close: async () => {
        if (closed) return;
        closed = true; channel.close();
        await invokeNative<DesktopAttachmentResult>("codex.desktop.attachments", { request: { revision, sessionId, action: { type: "unbind", bindingId: result.bindingId } } satisfies DesktopAttachmentRequest, onEvent: channel.value });
      }
    };
  } catch (error) { closed = true; channel.close(); throw error; }
}
