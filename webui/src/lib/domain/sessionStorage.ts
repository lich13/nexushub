import type { SessionStorageSize } from "../../types";

export function formatSessionStorage(size?: SessionStorageSize | null): { label: string; tone: "success" | "warning" | "danger" | "muted"; title: string } {
  const scope = size?.scope === "directory" ? "会话目录" : "会话文件";
  const unknown = { label: "—", tone: "muted" as const, title: "会话大小暂不可用" };
  if (size?.status === "pending") return { ...unknown, title: `${scope} · 正在统计` };
  if (!size || !["complete", "partial"].includes(size.status) || size.bytes === null || !Number.isSafeInteger(size.bytes) || size.bytes < 0) return unknown;
  const bytes = size.bytes;
  const unit = bytes >= 1024 ** 3 ? "G" : bytes >= 1024 ** 2 ? "M" : "K";
  const divisor = unit === "G" ? 1024 ** 3 : unit === "M" ? 1024 ** 2 : 1024;
  // Truncate at one decimal so a lower-bound partial result never rounds upward.
  const amount = Math.floor(bytes / divisor * 10) / 10;
  let label = bytes > 0 && bytes < 1024 ? "<1K" : `${amount}${unit}`;
  const exact = bytes.toLocaleString("en-US");
  if (size.status === "partial") label = bytes > 0 && bytes < 1024 ? "≥0K" : `≥${label}`;
  return { label, tone: unit === "G" ? "danger" : unit === "M" ? "warning" : "success",
    title: size.status === "partial" ? `${scope} · 至少 ${exact} 字节（统计不完整）` : `${scope} · ${exact} 字节` };
}
