import type {
  ArchiveDeletePlan,
  ArchiveDeleteResult,
  AgentProviderInfo,
  BridgeActionResult,
  ClaudeOverview,
  CodexConfig,
  CodexGoal,
  CodexGoalSaveInput,
  CodexModel,
  FollowUpQueueItem,
  FollowUpQueueState,
  HiddenThreadDeletePlan,
  HiddenThreadDeleteResult,
  JobRecord,
  MessageBlock,
  OptionalResult,
  PermissionProfile,
  PlatformOverview,
  PluginInfo,
  ProbeEventsResponse,
  ProbeJobAction,
  ProbeLogsDbStatus,
  ProbeSettings,
  ProbeStatus,
  PublicSettings,
  SecuritySettings,
  SentinelStatus,
  SessionUser,
  SystemStatus,
  SystemVersion,
  ThreadBlockPage,
  ThreadDetail,
  ThreadSummary,
  UpdateStatus,
  UploadOutcome
} from "../../types";
import {
  RuntimeUnavailableError,
  createRuntimeThreadEventSource,
  desktopSessionUser,
  runtimeDispatch,
  runtimeRpc,
  runtimeValue,
  uploadRuntimeFiles
} from "./transport";
import { USE_DEMO } from "./shared";
import { demoCodexGoal, demoThreads } from "./demo";

export async function listThreads(status: string, q: string): Promise<ThreadSummary[]> {
  if (USE_DEMO) return demoThreads(status, q);
  return runtimeDispatch<ThreadSummary[]>({
    command: "listThreads",
    args: { status, q, limit: 120 }
  });
}

export type ThreadDetailOptions = {
  limit?: number;
  before?: string | null;
  full?: boolean;
};

export async function getThread(id: string, options: ThreadDetailOptions = {}): Promise<ThreadDetail> {
  if (USE_DEMO) {
    const summary = demoThreads("all", "").find((thread) => thread.id === id) ?? demoThreads("all", "")[0];
    const longChatBlocks: MessageBlock[] = Array.from({ length: 68 }, (_, index) => ({
      id: `history-${index}`,
      role: index % 2 === 0 ? "user" : "assistant",
      kind: "message",
      text: index % 2 === 0 ? `历史请求 ${index + 1}` : `历史回复 ${index + 1}`,
      questions: []
    }));
    const completedTools: MessageBlock[] = Array.from({ length: 20 }, (_, index) => ({
      id: `tool-history-${index}`,
      role: "tool",
      kind: "function_call_output",
      tool_name: "shell",
      status: "completed",
      summary: `历史工具 ${index + 1} 已完成`,
      text: `stdout line ${index + 1}`,
      questions: []
    }));
    return {
      summary: id === "019e95a0-demo" ? { ...summary, active_turn_id: "turn-plan-demo", pending_elicitation: { turn_id: "turn-plan-demo", item_id: "question-demo", questions: [{ id: "q1", question: "选择执行方式", options: [{ label: "直接实施", description: "使用当前计划继续执行" }, { label: "先修改", description: "补充约束后重新计划" }] }] } } : summary,
      raw_event_count: 96,
      total_blocks: 96,
      has_more_blocks: false,
      before_cursor: null,
      blocks: [
        ...longChatBlocks,
        { id: "u1", role: "user", kind: "userMessage", text: "检查云机 Codex 状态。", questions: [] },
        { id: "plan-demo", role: "assistant", kind: "plan", display_kind: "plan", turn_id: "turn-plan-demo", item_id: "plan-demo", status: "pending", resolved: false, plan_status: "pending", text: "<proposed_plan>1. 核对线程状态\n2. 修复 Plan/Questions 展示\n3. 验证并部署</proposed_plan>", questions: [] },
        { id: "question-answered", role: "assistant", kind: "request_user_input_result", display_kind: "question_result", turn_id: "turn-old-demo", status: "completed", resolved: true, answers: [{ question_id: "q0", answers: ["保留"], note: "历史选择已回答" }], questions: [{ id: "q0", question: "历史选项", options: [{ label: "保留" }, { label: "修改" }] }] },
        { id: "question-demo", role: "assistant", kind: "request_user_input", display_kind: "question", turn_id: "turn-plan-demo", call_id: "question-demo", status: "pending", resolved: false, questions: [{ id: "q1", question: "选择执行方式", options: [{ label: "直接实施", description: "使用当前计划继续执行" }, { label: "先修改", description: "补充约束后重新计划" }] }] },
        { id: "a1", role: "assistant", kind: "agentMessage", text: "状态正常，本地 Codex 状态库可读。归档删除 dry-run 可执行。", questions: [] },
        ...completedTools,
        { id: "t1", role: "tool", kind: "commandExecution", tool_name: "shell", text: "codex-cloud-doctor\nsqlite integrity_check: ok", status: "completed", questions: [] },
        { id: "t-running", role: "tool", kind: "function_call", tool_name: "shell", summary: "正在刷新本地状态", text: "sqlite3 /root/.codex/state_5.sqlite 'pragma integrity_check;'", status: "running", questions: [] }
      ],
      messages: [
        { role: "user", kind: "message", text: "检查云机 Codex 状态。" },
        { role: "assistant", kind: "message", text: "状态正常，本地 Codex 状态库可读。归档删除 dry-run 可执行。" },
        { role: "tool", kind: "function_call", text: "codex-cloud-doctor\nsqlite integrity_check: ok" }
      ]
    };
  }
  return runtimeDispatch<ThreadDetail>({
    command: "getThread",
    args: { id, options }
  });
}

export async function getThreadBlocks(id: string, options: Pick<ThreadDetailOptions, "limit" | "before"> = {}): Promise<ThreadBlockPage> {
  if (USE_DEMO) {
    const detail = await getThread(id, options);
    return {
      thread_id: id,
      blocks: detail.blocks,
      total_blocks: detail.total_blocks ?? detail.blocks.length,
      has_more_blocks: Boolean(detail.has_more_blocks),
      before_cursor: detail.before_cursor ?? null
    };
  }
  return runtimeDispatch<ThreadBlockPage>({
    command: "getThreadBlocks",
    args: { id, options }
  });
}

export type ThreadSendPayload = {
  message: string;
  attachments?: string[];
  model?: string | null;
  service_tier?: string | null;
  reasoning_effort?: string | null;
  cwd?: string | null;
  permission_profile?: string | null;
  approval_policy?: string | null;
  sandbox_mode?: string | null;
  network_access?: boolean | null;
  collaboration_mode?: string | null;
};

export async function uploadFiles(files: File[], csrfToken?: string | null): Promise<UploadOutcome> {
  if (USE_DEMO) {
    return {
      files: files.map((file, index) => ({
        id: `upload-demo-${Date.now()}-${index}`,
        name: file.name,
        mime: file.type || "application/octet-stream",
        size: file.size,
        sha256: "demo",
        kind: file.type.startsWith("image/") ? "image" : file.name.endsWith(".md") ? "markdown" : "text",
        status: "ready"
      }))
    };
  }
  return uploadRuntimeFiles<UploadOutcome>(files, csrfToken);
}

export async function deleteUpload(id: string, csrfToken?: string | null): Promise<{ ok: boolean; deleted: boolean }> {
  if (USE_DEMO) return { ok: true, deleted: true };
  return runtimeDispatch<{ ok: boolean; deleted: boolean }>({
    command: "deleteUpload",
    args: { id, csrfToken }
  });
}

export async function createThread(payload: ThreadSendPayload, csrfToken?: string | null): Promise<BridgeActionResult> {
  if (USE_DEMO) return { bridge: false, thread_id: "019e-new-demo", turn_id: "turn-demo", fallback: true, message: "已提交给 Codex" };
  return runtimeDispatch<BridgeActionResult>({
    command: "createThread",
    args: { payload, csrfToken }
  });
}

export async function sendMessage(threadId: string, payload: ThreadSendPayload, csrfToken?: string | null): Promise<BridgeActionResult> {
  if (USE_DEMO) return { bridge: false, thread_id: threadId, turn_id: "turn-demo", fallback: true, message: "已提交给 Codex" };
  return runtimeDispatch<BridgeActionResult>({
    command: "sendMessage",
    args: { threadId, payload, csrfToken }
  });
}

export async function steerThread(threadId: string, payload: ThreadSendPayload, csrfToken?: string | null): Promise<BridgeActionResult> {
  if (USE_DEMO) return { bridge: false, thread_id: threadId, turn_id: "turn-demo", fallback: true, message: "已提交给 Codex" };
  return runtimeDispatch<BridgeActionResult>({
    command: "steerThread",
    args: { threadId, payload, csrfToken }
  });
}

export async function listFollowUps(threadId: string): Promise<FollowUpQueueState> {
  if (USE_DEMO) return { items: [] };
  const result = await runtimeDispatch<FollowUpQueueState | FollowUpQueueItem[]>({
    command: "listFollowUps",
    args: { threadId }
  });
  return Array.isArray(result) ? { items: result } : result;
}

export async function enqueueFollowUp(threadId: string, payload: ThreadSendPayload, csrfToken?: string | null): Promise<FollowUpQueueItem> {
  if (USE_DEMO) {
    return {
      id: `follow-up-${Date.now()}`,
      thread_id: threadId,
      status: "pending",
      message: payload.message,
      options: payload,
      created_at: Math.floor(Date.now() / 1000)
    };
  }
  return runtimeDispatch<FollowUpQueueItem>({
    command: "enqueueFollowUp",
    args: { threadId, payload, csrfToken }
  });
}

export async function cancelFollowUp(threadId: string, followUpId: string, csrfToken?: string | null): Promise<{ ok: boolean }> {
  if (USE_DEMO) return { ok: true };
  return runtimeDispatch<{ ok: boolean }>({
    command: "cancelFollowUp",
    args: { threadId, followUpId, csrfToken }
  });
}

export async function stopThread(threadId: string, payload: { turn_id?: string | null; job_id?: string | null }, csrfToken?: string | null) {
  if (USE_DEMO) return { ok: true };
  return runtimeDispatch({
    command: "stopThread",
    args: { threadId, payload, csrfToken }
  });
}

export async function archiveThread(threadId: string, csrfToken?: string | null) {
  return runtimeDispatch({
    command: "archiveThread",
    args: { threadId, csrfToken }
  });
}

export async function restoreThread(threadId: string, csrfToken?: string | null) {
  return runtimeDispatch({
    command: "restoreThread",
    args: { threadId, csrfToken }
  });
}

export async function renameThread(threadId: string, name: string, csrfToken?: string | null) {
  return runtimeDispatch({
    command: "renameThread",
    args: { threadId, name, csrfToken }
  });
}

export async function forkThread(threadId: string, csrfToken?: string | null): Promise<BridgeActionResult> {
  return runtimeDispatch<BridgeActionResult>({
    command: "forkThread",
    args: { threadId, csrfToken },
    desktopUnavailable: "Desktop fork command is not implemented"
  });
}

export async function answerElicitation(threadId: string, answers: Record<string, string[]>, csrfToken?: string | null): Promise<BridgeActionResult> {
  return runtimeDispatch<BridgeActionResult>({
    command: "answerElicitation",
    args: { threadId, answers, csrfToken }
  });
}

export async function acceptPlan(
  threadId: string,
  payload: { turn_id?: string | null; item_id?: string | null },
  csrfToken?: string | null
): Promise<BridgeActionResult> {
  return runtimeDispatch<BridgeActionResult>({
    command: "acceptPlan",
    args: { threadId, payload, csrfToken }
  });
}

export async function revisePlan(
  threadId: string,
  payload: { turn_id?: string | null; item_id?: string | null; instructions: string },
  csrfToken?: string | null
): Promise<BridgeActionResult> {
  return runtimeDispatch<BridgeActionResult>({
    command: "revisePlan",
    args: { threadId, payload, csrfToken }
  });
}

export async function answerApproval(
  threadId: string,
  payload: { turn_id?: string | null; item_id?: string | null; request_id?: string | null; decision: string },
  csrfToken?: string | null
): Promise<BridgeActionResult> {
  return runtimeDispatch<BridgeActionResult>({
    command: "answerApproval",
    args: { threadId, payload, csrfToken },
    desktopUnavailable: "Desktop approval command is not implemented"
  });
}

export async function getCodexGoal(threadId: string): Promise<CodexGoal> {
  if (USE_DEMO) return demoCodexGoal(threadId);
  const result = await runtimeDispatch<CodexGoal | { goal?: CodexGoal | null }>({
    command: "getCodexGoal",
    args: { threadId }
  });
  return result && typeof result === "object" && "goal" in result
    ? result.goal ?? demoCodexGoal(threadId)
    : result as CodexGoal;
}

export async function saveCodexGoal(threadId: string, goal: CodexGoalSaveInput, csrfToken?: string | null): Promise<CodexGoal> {
  if (USE_DEMO) {
    return {
      ...demoCodexGoal(threadId),
      enabled: true,
      objective: goal.objective.trim(),
      token_budget: goal.token_budget ?? null,
      status: "active"
    };
  }
  return runtimeDispatch<CodexGoal>({
    command: "saveCodexGoal",
    args: {
      threadId,
      objective: goal.objective,
      tokenBudget: goal.token_budget ?? null,
      csrfToken
    }
  });
}

export async function clearCodexGoal(threadId: string, csrfToken?: string | null): Promise<CodexGoal> {
  if (USE_DEMO) {
    return {
      ...demoCodexGoal(threadId),
      enabled: false,
      objective: null,
      token_budget: null,
      status: "cleared"
    };
  }
  return runtimeDispatch<CodexGoal>({
    command: "clearCodexGoal",
    args: { threadId, csrfToken }
  });
}

export async function pauseCodexGoal(threadId: string, csrfToken?: string | null): Promise<CodexGoal> {
  if (USE_DEMO) {
    return {
      ...demoCodexGoal(threadId),
      enabled: true,
      status: "paused"
    };
  }
  return runtimeDispatch<CodexGoal>({
    command: "pauseCodexGoal",
    args: { threadId, csrfToken }
  });
}

export async function resumeCodexGoal(threadId: string, csrfToken?: string | null): Promise<CodexGoal> {
  if (USE_DEMO) {
    return {
      ...demoCodexGoal(threadId),
      enabled: true,
      status: "active"
    };
  }
  return runtimeDispatch<CodexGoal>({
    command: "resumeCodexGoal",
    args: { threadId, csrfToken }
  });
}

export function subscribeThreadEvents(
  threadId: string,
  handlers: { onBlock?: (block: MessageBlock, threadId: string) => void; onBlocks?: (blocks: MessageBlock[], threadId: string) => void; onSummary?: (summary: ThreadSummary, threadId: string) => void; onError?: (message: string, threadId: string) => void }
): () => void {
  if (USE_DEMO) return () => {};
  const source = createRuntimeThreadEventSource(threadId);
  if (source.unavailable) return () => {};
  let pendingBlocks: MessageBlock[] = [];
  let flushTimer: ReturnType<typeof setTimeout> | null = null;
  const flushBlocks = () => {
    flushTimer = null;
    if (!pendingBlocks.length) return;
    const blocks = pendingBlocks;
    pendingBlocks = [];
    handlers.onBlocks?.(blocks, threadId);
  };
  source.addEventListener("block", (event) => {
    const block = JSON.parse((event as MessageEvent).data) as MessageBlock;
    handlers.onBlock?.(block, threadId);
    if (handlers.onBlocks) {
      pendingBlocks.push(block);
      if (!flushTimer) flushTimer = setTimeout(flushBlocks, 100);
    }
  });
  source.addEventListener("summary", (event) => handlers.onSummary?.(JSON.parse((event as MessageEvent).data), threadId));
  source.addEventListener("error", (event) => {
    const data = (event as MessageEvent).data;
    handlers.onError?.(data ? String(data) : "stream disconnected", threadId);
  });
  return () => {
    if (flushTimer) {
      clearTimeout(flushTimer);
      flushBlocks();
    }
    source.close();
  };
}
