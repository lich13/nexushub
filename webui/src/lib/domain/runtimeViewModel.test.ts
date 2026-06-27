import { describe, expect, test } from "vitest";
import type {
  ArchiveDeletePlan,
  HiddenThreadDeletePlan,
  HiddenThreadDeleteResult,
  SystemStatus,
  UpdateStatus
} from "../../types";
import type { RuntimeCapabilityMatrix } from "./capabilities";
import {
  canStartHiddenThreadDelete,
  failureCategoryLabel,
  opsUpdateActionView,
  opsWorkspacePanelTitles,
  opsWorkspaceViewModel,
  threadMessageControllerView
} from "./runtimeViewModel";

const linuxWebCapabilities: RuntimeCapabilityMatrix = {
  runtimeKind: "web",
  hostSurface: "linux_server_webui",
  webAuth: true,
  logout: true,
  securitySettings: true,
  publicEndpointStatus: true,
  codexStatePaths: true,
  updatePrune: true,
  threadCleanup: true,
  probeLogMaintenance: true,
  threadArchiveActions: true,
  updateServiceLabels: true,
  desktopWebuiControl: false,
  forkAction: true,
  approvalActions: true
};

const macosTauriCapabilities: RuntimeCapabilityMatrix = {
  runtimeKind: "desktop",
  hostSurface: "desktop_embedded_tauri",
  webAuth: false,
  logout: false,
  securitySettings: false,
  publicEndpointStatus: false,
  codexStatePaths: false,
  updatePrune: false,
  threadCleanup: true,
  probeLogMaintenance: true,
  threadArchiveActions: true,
  updateServiceLabels: false,
  desktopWebuiControl: true,
  forkAction: false,
  approvalActions: false
};

function archivePlan(): ArchiveDeletePlan {
  return {
    total_threads: 9,
    active_threads: 7,
    archived_threads: 2,
    session_index_lines: 9,
    rollout_files: 9,
    archived_ids: ["a", "b"],
    integrity: "ok"
  };
}

function hiddenPlan(): HiddenThreadDeletePlan {
  return {
    total_threads: 9,
    visible_threads: 7,
    hidden_threads: 2,
    archived_threads: 0,
    session_index_lines: 9,
    rollout_files: 9,
    hidden_ids: ["child-a", "child-b"],
    hidden_source_counts: { subagent: 2 },
    integrity: "ok"
  };
}

describe("runtime view-model helpers", () => {
  test("derives Ops workspace copy and gating from capabilities without leaking Linux labels to macOS", () => {
    const updateStatus: UpdateStatus = {
      current_version: "0.1.100",
      latest_version: "v0.1.103",
      update_available: true,
      channel: "stable",
      method: "macos_tauri_updater",
      state: "idle",
      failure_category: null,
      recommended_action: "Confirm install in the Tauri updater.",
      capabilities: ["check", "confirm_install"]
    };
    const systemStatus: SystemStatus = {
      hostname: "macos",
      public_endpoint: "https://661313.xyz/nexushub/",
      hidden_thread_count: 3,
      state_db_integrity: "ok"
    };
    const hiddenDeleteResult: HiddenThreadDeleteResult = {
      before: hiddenPlan(),
      deleted_threads: 2,
      after_total_threads: 7,
      after_visible_threads: 7,
      after_hidden_threads: 0,
      after_archived_threads: 0,
      after_integrity: "ok",
      visible_threads: 7,
      hidden_threads: 0,
      integrity: "ok",
      deleted_rollout_files: 2
    };

    const view = opsWorkspaceViewModel({
      capabilities: macosTauriCapabilities,
      status: systemStatus,
      updateStatus,
      archivePlan: archivePlan(),
      hiddenPlan: hiddenPlan(),
      hiddenDeleteResult,
      archiveCleanup: { dryRunPending: false, armed: true, executePending: false },
      hiddenCleanup: { dryRunPending: false, armed: false, executePending: false }
    });

    expect(view.panelTitles).toEqual(opsWorkspacePanelTitles(macosTauriCapabilities));
    expect(view.systemMetrics.map((metric) => metric.label)).toEqual(["Hostname", "Hidden threads", "Sources"]);
    expect(view.updateActions).toEqual(opsUpdateActionView(updateStatus, macosTauriCapabilities));
    expect(view.archiveCleanup.stage).toEqual({ label: "等待确认", tone: "danger" });
    expect(view.archiveCleanup.canArm).toBe(true);
    expect(view.hiddenCleanup.stats).toEqual({
      hidden: 2,
      visible: 7,
      sourceCounts: "subagent:2",
      integrity: "ok"
    });
    expect(view.hiddenCleanup.rolloutDeleteResult).toBe("2");
    expect(view.hiddenCleanup.canArm).toBe(canStartHiddenThreadDelete(hiddenPlan()));
    expect(JSON.stringify(view)).not.toMatch(/systemd|Nginx|Turnstile|管理员密码|Public endpoint|Linux prune|Prune|661313\.xyz/i);
  });

  test("keeps Linux service labels when capabilities advertise the Linux WebUI surface", () => {
    const view = opsWorkspaceViewModel({
      capabilities: linuxWebCapabilities,
      status: {
        hostname: "codex-cloud-root",
        public_endpoint: "https://661313.xyz/nexushub/",
        state_db_integrity: "ok",
        codex_home: "/root/.codex",
        state_db: "/var/lib/nexushub-webd/nexushub.sqlite",
        hidden_thread_count: 0
      },
      updateStatus: {
        current_version: "0.1.100",
        latest_version: "v0.1.103",
        update_available: false,
        channel: "stable",
        method: "linux_systemd_job",
        state: "idle",
        failure_category: "systemd_failure",
        recommended_action: "/usr/local/bin/nexushub-webd-update",
        capabilities: ["prune_backups"]
      },
      archivePlan: null,
      hiddenPlan: null,
      hiddenDeleteResult: null,
      archiveCleanup: { dryRunPending: false, armed: false, executePending: false },
      hiddenCleanup: { dryRunPending: true, armed: false, executePending: false }
    });

    expect(view.systemMetrics.map((metric) => metric.label)).toEqual([
      "Hostname",
      "Public endpoint",
      "state DB",
      "Codex Home",
      "State DB",
      "Hidden threads",
      "Sources"
    ]);
    expect(view.updateActions.map((action) => action.label)).toEqual(["Precheck", "Update", "Prune"]);
    expect(failureCategoryLabel("systemd_failure", linuxWebCapabilities)).toBe("systemd 失败");
    expect(view.hiddenCleanup.stage).toEqual({ label: "扫描中", tone: "warning" });
  });

  test("derives thread message controller inputs without exposing store/cache internals to components", () => {
    const selectedThreadSummary = {
      id: "thread-a",
      title: "Thread A",
      status: "Recent",
      message_count: 1
    } as const;
    const selectedDetail = {
      summary: selectedThreadSummary,
      messages: [],
      blocks: [],
      raw_event_count: 0
    };
    const view = threadMessageControllerView({
      threadId: "thread-a",
      selectedThreadSummary,
      selectedDetail
    });

    expect(view.hydration).toEqual({
      threadId: "thread-a",
      selectedThreadSummary,
      selectedDetail
    });
    expect(view.realtime.threadId).toBe("thread-a");
    expect(view.realtime.applyThreadTitleOverride).toBeTypeOf("function");
    expect(Object.keys(view).sort()).toEqual(["hydration", "realtime"]);
    expect(JSON.stringify(view)).not.toMatch(/queryClient|setQueryData|invalidateQueries|useQueryClient/);
  });
});
