import type { RuntimeCapabilityMatrix } from "./capabilities";

export type HostCapabilityPolicy = {
  showLinuxWebCapabilities: boolean;
  copyRedactionEnabled: boolean;
  failureLabels: Record<string, string>;
};

const linuxFailureLabels: Record<string, string> = {
  systemd_failure: "systemd 失败",
  nginx_failure: "Nginx 失败",
  permission_denied_sudo: "权限或 sudo 失败"
};

const genericFailureLabels: Record<string, string> = {
  systemd_failure: "服务失败",
  nginx_failure: "更新失败",
  permission_denied_sudo: "权限失败"
};

export function hostCapabilityPolicy(capabilities: RuntimeCapabilityMatrix): HostCapabilityPolicy {
  return capabilities.hostSurface === "linux_server_webui"
    ? {
      showLinuxWebCapabilities: true,
      copyRedactionEnabled: false,
      failureLabels: linuxFailureLabels
    }
    : {
      showLinuxWebCapabilities: false,
      copyRedactionEnabled: true,
      failureLabels: genericFailureLabels
    };
}

export function redactHostCopy(value: string, fallback: string): string {
  const sanitized = value
    .replace(/https?:\/\/[^\s)"']+/gi, "本机入口")
    .replace(/\b(?:\d{1,3}\.){3}\d{1,3}\b/g, "本机地址")
    .replace(/\b[a-z0-9.-]*661313\.xyz\b/gi, "本机域名")
    .replace(/\/opt\/nexushub[^\s)"']*/gi, "本机路径")
    .replace(/\/root\/\.codex[^\s)"']*/gi, "本机 Codex 目录")
    .replace(/\/home\/ubuntu[^\s)"']*/gi, "本机工作区")
    .replace(/\bturnstile\b/gi, "验证")
    .replace(/公网入口/g, "本机入口")
    .replace(/\bsystemd\b/gi, "服务")
    .replace(/\bnginx\b/gi, "服务")
    .replace(/管理员密码/g, "权限")
    .replace(/Linux prune/gi, "清理")
    .replace(/Linux update/gi, "更新")
    .replace(/\bLinux\b/g, "当前宿主")
    .replace(/\bsudo\b/gi, "权限")
    .trim();
  return /systemd|nginx|turnstile|管理员密码|Linux prune|Linux update|sudo|661313\.xyz|43\.155\.235\.227|\/opt\/nexushub|\/root\/\.codex|\/home\/ubuntu/i.test(sanitized)
    ? fallback
    : sanitized || fallback;
}
