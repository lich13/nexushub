import { ExternalLink, Target } from "lucide-react";
import type { DesktopGoal as Goal } from "../../lib/query/desktop";

const labels: Record<string, string> = {
  active: "进行中的目标", paused: "目标已暂停", blocked: "目标受阻",
  usageLimited: "目标受用量限制", budgetLimited: "目标受预算限制", complete: "目标已完成"
};

/** The native snapshot owns this state; the existing open action owns editing. */
export function DesktopGoal({ goal, open, busy }: { goal: Goal; open: () => Promise<void>; busy: boolean }) {
  return <div className="desktop-native-goal">
    <Target size={16} />
    <details>
      <summary><strong>{labels[goal.status] ?? "目标状态待确认"}</strong><span>{goal.objective}</span></summary>
      <p>{goal.objective}</p>
      {goal.truncated && <small>完整目标请在 Codex 中查看</small>}
    </details>
    <button type="button" className="icon-button compact" title="在 Codex 中管理目标" aria-label="在 Codex 中管理目标" disabled={busy} onClick={() => { void open(); }}><ExternalLink size={14} /></button>
  </div>;
}
