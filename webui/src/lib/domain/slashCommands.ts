import { capabilitiesForInput, type RuntimeCapabilityInput } from "./runtimeViewModel";

export type SlashCommand = {
  command: string;
  description: string;
  usageHint: string;
  requiresThread?: boolean;
};

export type SlashCommandAction =
  | { kind: "archive_thread" | "copy_latest" | "fork_thread" | "open_debug_config" | "open_new_thread" | "open_plugins" | "open_resume" | "open_status" | "open_thread_settings" | "stop_thread" | "toggle_fast" | "toggle_plan_mode"; command: string; message?: string }
  | { kind: "focus_control" | "insert_template" | "requires_thread" | "unavailable" | "unknown"; command: string; message: string };

type ControlledSlashActionKind = "archive_thread" | "copy_latest" | "fork_thread" | "open_debug_config" | "open_new_thread" | "open_plugins" | "open_resume" | "open_status" | "open_thread_settings" | "stop_thread" | "toggle_fast" | "toggle_plan_mode";

export type SlashCommandInspectorState = {
  showFork: boolean;
  showArchive: boolean;
  approvalMode: "interactive" | "unsupported";
};

export type SlashCommandExecutionPlan =
  | { kind: "toggle_plan_mode"; draft: ""; message: string }
  | { kind: "open_plugins"; draft: ""; message: string }
  | { kind: "open_status"; draft: ""; message: string }
  | { kind: "open_new_thread"; draft: ""; message?: string }
  | { kind: "open_resume"; draft: ""; message: string }
  | { kind: "open_thread_settings"; draft: ""; message: string }
  | { kind: "archive_thread"; draft: "" }
  | { kind: "fork_thread"; draft: "" }
  | { kind: "stop_thread"; draft: "" }
  | { kind: "copy_latest"; draft: ""; text: string; message: string }
  | { kind: "toggle_fast"; draft: ""; serviceTier: string; message: string }
  | { kind: "insert_template"; draft: string; message: string }
  | { kind: "feedback"; draft: ""; message: string };

export type SlashCommandExecutionPlanInput = {
  command: string;
  hasThread?: boolean;
  capabilities?: RuntimeCapabilityInput;
  inspectorActions: SlashCommandInspectorState;
  supportsFast: boolean;
  serviceTier: string;
  latestAssistantCopy?: string | null;
};

export const slashCommands: SlashCommand[] = [
  { command: "/permissions", description: "调整权限与审批模式", usageHint: "/permissions" },
  { command: "/ide", description: "加入 IDE 上下文", usageHint: "/ide" },
  { command: "/keymap", description: "查看或调整 TUI 快捷键", usageHint: "/keymap" },
  { command: "/vim", description: "切换 Vim 输入模式", usageHint: "/vim" },
  { command: "/sandbox-add-read-dir", description: "添加沙盒只读目录，Windows 专用", usageHint: "/sandbox-add-read-dir <path>" },
  { command: "/agent", description: "切换或查看子代理线程", usageHint: "/agent", requiresThread: true },
  { command: "/apps", description: "浏览 apps 与 connectors", usageHint: "/apps" },
  { command: "/plugins", description: "浏览插件", usageHint: "/plugins" },
  { command: "/hooks", description: "查看生命周期 hooks", usageHint: "/hooks" },
  { command: "/clear", description: "清空当前输入或开启新会话语义", usageHint: "/clear" },
  { command: "/archive", description: "归档当前会话", usageHint: "/archive", requiresThread: true },
  { command: "/compact", description: "压缩当前上下文", usageHint: "/compact", requiresThread: true },
  { command: "/copy", description: "复制最新回复", usageHint: "/copy", requiresThread: true },
  { command: "/diff", description: "查看当前 diff", usageHint: "/diff", requiresThread: true },
  { command: "/exit", description: "退出当前会话", usageHint: "/exit", requiresThread: true },
  { command: "/quit", description: "退出当前会话", usageHint: "/quit", requiresThread: true },
  { command: "/experimental", description: "查看实验功能", usageHint: "/experimental" },
  { command: "/approve", description: "批准一次自动审查拒绝后的重试", usageHint: "/approve", requiresThread: true },
  { command: "/memories", description: "查看记忆设置", usageHint: "/memories" },
  { command: "/skills", description: "浏览或使用技能", usageHint: "/skills" },
  { command: "/feedback", description: "提交反馈", usageHint: "/feedback" },
  { command: "/init", description: "生成 AGENTS.md", usageHint: "/init" },
  { command: "/logout", description: "退出登录", usageHint: "/logout" },
  { command: "/mcp", description: "查看 MCP 工具", usageHint: "/mcp" },
  { command: "/mention", description: "引用文件或目录", usageHint: "/mention <path>" },
  { command: "/model", description: "切换模型或推理等级", usageHint: "/model" },
  { command: "/fast", description: "切换 Fast 服务层", usageHint: "/fast" },
  { command: "/plan", description: "切换计划模式，可带内联提示", usageHint: "/plan [prompt]" },
  { command: "/personality", description: "切换沟通风格", usageHint: "/personality" },
  { command: "/ps", description: "查看后台终端", usageHint: "/ps", requiresThread: true },
  { command: "/stop", description: "停止后台终端", usageHint: "/stop", requiresThread: true },
  { command: "/fork", description: "分叉当前对话", usageHint: "/fork", requiresThread: true },
  { command: "/side", description: "开启旁路对话", usageHint: "/side [prompt]", requiresThread: true },
  { command: "/btw", description: "开启旁路对话别名", usageHint: "/btw [prompt]", requiresThread: true },
  { command: "/raw", description: "切换原始滚动输出", usageHint: "/raw", requiresThread: true },
  { command: "/resume", description: "恢复历史会话", usageHint: "/resume" },
  { command: "/new", description: "新建会话", usageHint: "/new" },
  { command: "/review", description: "请求工作区 review", usageHint: "/review" },
  { command: "/status", description: "查看会话状态", usageHint: "/status" },
  { command: "/debug-config", description: "查看配置层诊断", usageHint: "/debug-config" },
  { command: "/statusline", description: "配置状态栏", usageHint: "/statusline" },
  { command: "/title", description: "配置终端标题", usageHint: "/title" },
  { command: "/theme", description: "选择语法主题", usageHint: "/theme" }
];

const desktopUnsupportedSlashCommands = new Set(["/fork"]);

export function slashCommandsForRuntime(input?: RuntimeCapabilityInput): SlashCommand[] {
  const capabilities = capabilitiesForInput(input);
  return slashCommands.filter((item) => {
    if (!capabilities.forkAction && desktopUnsupportedSlashCommands.has(item.command)) return false;
    if (!capabilities.threadArchiveActions && item.command === "/archive") return false;
    if (!capabilities.logout && item.command === "/logout") return false;
    return true;
  });
}

const controlledSlashActions: Record<string, ControlledSlashActionKind> = {
  "/new": "open_new_thread",
  "/resume": "open_resume",
  "/archive": "archive_thread",
  "/fork": "fork_thread",
  "/stop": "stop_thread",
  "/fast": "toggle_fast",
  "/plan": "toggle_plan_mode",
  "/status": "open_status",
  "/debug-config": "open_debug_config",
  "/copy": "copy_latest",
  "/plugins": "open_plugins",
  "/apps": "open_plugins",
  "/skills": "open_plugins"
};

const focusSlashCommands = new Set(["/model", "/permissions", "/title"]);
const templateSlashCommands = new Set(["/compact", "/diff", "/mention", "/review", "/side", "/btw", "/raw", "/init", "/approve"]);
const unavailableSlashCommands: Record<string, string> = {
  "/ide": "Web 端暂不支持注入 IDE 上下文；可在本机 Codex TUI 中使用该命令。",
  "/vim": "Web 端暂不支持 Vim 输入模式；请使用浏览器输入法或本机 TUI。",
  "/keymap": "Web 端暂不支持 TUI 快捷键设置；浏览器快捷键由系统和浏览器管理。",
  "/theme": "Web 端暂不支持 TUI 主题切换；NexusHub 使用固定设计系统。",
  "/exit": "Web 端暂不需要退出 TUI；关闭页面或切换线程即可。",
  "/quit": "Web 端暂不需要退出 TUI；关闭页面或切换线程即可。",
  "/sandbox-add-read-dir": "Web 端暂不支持动态添加沙盒只读目录；请通过 Codex 配置或受控权限预设处理。",
  "/agent": "Web 端暂不支持切换子代理控制台；可在线程列表查看主线程。",
  "/hooks": "Web 端 Hook 维护在探针页面处理。",
  "/clear": "Web 端不清空历史线程；可清空当前输入或新建线程。",
  "/experimental": "Web 端暂不暴露实验开关。",
  "/memories": "Web 端暂不管理本机记忆；请在 Codex TUI 或本地文件中查看。",
  "/feedback": "Web 端暂不接入反馈通道。",
  "/logout": "请使用左下角退出登录按钮。",
  "/mcp": "Web 端暂不直接操作 MCP；可在插件/Provider 页面查看可用能力。",
  "/personality": "Web 端暂不支持切换 Personality。请在 Codex 配置中调整。",
  "/ps": "Web 端暂不显示后台终端列表；当前固定运维任务在 Job History 中查看。",
  "/statusline": "Web 端暂不配置 TUI 状态栏。"
};

export function slashCommandAction(command: string, hasThread = true, input?: RuntimeCapabilityInput): SlashCommandAction {
  const normalized = command.trim().replace(/\s+/g, " ");
  const known = slashCommandsForRuntime(input).find((item) => item.command === normalized);
  if (!known) return { kind: "unknown", command: normalized, message: "未知 Slash 命令" };
  if (known.requiresThread && !hasThread) {
    return { kind: "requires_thread", command: normalized, message: "该命令需要已有线程，请先选择或创建线程。" };
  }
  const controlled = controlledSlashActions[normalized];
  if (controlled) return { kind: controlled, command: normalized };
  if (focusSlashCommands.has(normalized)) {
    return { kind: "focus_control", command: normalized, message: "请使用输入框下方的同名控制项调整。" };
  }
  if (templateSlashCommands.has(normalized)) {
    return { kind: "insert_template", command: normalized, message: "已插入命令模板，请补充参数后发送。" };
  }
  return {
    kind: "unavailable",
    command: normalized,
    message: unavailableSlashCommands[normalized] ?? "Web 端暂不支持该 TUI 命令；请使用现有面板或本机 Codex TUI。"
  };
}

export function slashCommandExecutionPlan(input: SlashCommandExecutionPlanInput): SlashCommandExecutionPlan {
  const normalized = input.command.trim().replace(/\s+/g, " ");
  const action = slashCommandAction(normalized, input.hasThread ?? true, input.capabilities);
  if (action.kind === "unknown") {
    if (normalized === "/fork" && !input.inspectorActions.showFork) {
      return { kind: "feedback", draft: "", message: "macOS App 当前不支持 Fork 操作" };
    }
    if (normalized === "/archive" && !input.inspectorActions.showArchive) {
      return { kind: "feedback", draft: "", message: "当前运行时不支持归档操作" };
    }
  }
  switch (action.kind) {
    case "toggle_plan_mode":
      return { kind: "toggle_plan_mode", draft: "", message: action.message ?? "Plan Mode 已切换" };
    case "open_plugins":
      return { kind: "open_plugins", draft: "", message: action.message ?? "已打开插件/Provider 面板" };
    case "open_status":
      return { kind: "open_status", draft: "", message: action.message ?? "已打开线程状态" };
    case "open_new_thread":
      return { kind: "open_new_thread", draft: "" };
    case "open_resume":
      return { kind: "open_resume", draft: "", message: "请在线程列表选择要恢复的会话" };
    case "open_thread_settings":
      return { kind: "open_thread_settings", draft: "", message: "线程设置已在右侧面板显示" };
    case "archive_thread":
      return input.inspectorActions.showArchive
        ? { kind: "archive_thread", draft: "" }
        : { kind: "feedback", draft: "", message: "当前运行时不支持归档操作" };
    case "fork_thread":
      return input.inspectorActions.showFork
        ? { kind: "fork_thread", draft: "" }
        : { kind: "feedback", draft: "", message: "macOS App 当前不支持 Fork 操作" };
    case "stop_thread":
      return { kind: "stop_thread", draft: "" };
    case "copy_latest":
      return input.latestAssistantCopy
        ? { kind: "copy_latest", draft: "", text: input.latestAssistantCopy, message: "已复制最新回复" }
        : { kind: "feedback", draft: "", message: "没有可复制的最新回复" };
    case "toggle_fast":
      if (!input.supportsFast) {
        return { kind: "feedback", draft: "", message: "当前模型不支持 Fast service tier" };
      }
      {
        const next = input.serviceTier === "priority" ? "" : "priority";
        return {
          kind: "toggle_fast",
          draft: "",
          serviceTier: next,
          message: next === "priority" ? "Fast 已开启" : "Fast 已关闭"
        };
      }
    case "insert_template":
      return { kind: "insert_template", draft: `${action.command} `, message: action.message };
    case "focus_control":
    case "requires_thread":
    case "unavailable":
    case "unknown":
    default:
      return { kind: "feedback", draft: "", message: action.message ?? "已执行" };
  }
}
