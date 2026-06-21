pub const AUTH_PUBLIC_SETTINGS: &str = "auth.publicSettings";
pub const AUTH_LOGIN: &str = "auth.login";
pub const AUTH_LOGOUT: &str = "auth.logout";
pub const AUTH_ME: &str = "auth.me";
pub const SECURITY_GET: &str = "security.get";
pub const SECURITY_SAVE: &str = "security.save";
pub const SECURITY_CHANGE_PASSWORD: &str = "security.changePassword";
pub const SYSTEM_STATUS: &str = "system.status";
pub const SYSTEM_VERSION: &str = "system.version";
pub const SYSTEM_PLATFORM: &str = "system.platform";
pub const SYSTEM_PROVIDERS: &str = "system.providers";
pub const SYSTEM_PLUGINS: &str = "system.plugins";
pub const SYSTEM_MODELS: &str = "system.models";
pub const SYSTEM_PERMISSION_PROFILES: &str = "system.permissionProfiles";
pub const SYSTEM_CODEX_CONFIG: &str = "system.codexConfig";
pub const SYSTEM_CLAUDE_CODE_OVERVIEW: &str = "system.claudeCodeOverview";
pub const THREADS_LIST: &str = "threads.list";
pub const THREADS_DETAIL: &str = "threads.detail";
pub const THREADS_BLOCKS: &str = "threads.blocks";
pub const THREADS_CREATE: &str = "threads.create";
pub const THREADS_SEND: &str = "threads.send";
pub const THREADS_STEER: &str = "threads.steer";
pub const THREADS_STOP: &str = "threads.stop";
pub const THREADS_ARCHIVE: &str = "threads.archive";
pub const THREADS_RESTORE: &str = "threads.restore";
pub const THREADS_RENAME: &str = "threads.rename";
pub const THREADS_FORK: &str = "threads.fork";
pub const THREADS_FOLLOWUPS_LIST: &str = "threads.followups.list";
pub const THREADS_FOLLOWUPS_ENQUEUE: &str = "threads.followups.enqueue";
pub const THREADS_FOLLOWUPS_CANCEL: &str = "threads.followups.cancel";
pub const THREADS_PLAN_ACCEPT: &str = "threads.plan.accept";
pub const THREADS_PLAN_REVISE: &str = "threads.plan.revise";
pub const THREADS_ELICITATION_ANSWER: &str = "threads.elicitation.answer";
pub const THREADS_APPROVAL_ANSWER: &str = "threads.approval.answer";
pub const THREADS_GOAL_GET: &str = "threads.goal.get";
pub const THREADS_GOAL_SAVE: &str = "threads.goal.save";
pub const THREADS_GOAL_CLEAR: &str = "threads.goal.clear";
pub const THREADS_GOAL_PAUSE: &str = "threads.goal.pause";
pub const THREADS_GOAL_RESUME: &str = "threads.goal.resume";
pub const JOBS_LIST: &str = "jobs.list";
pub const JOBS_DETAIL: &str = "jobs.detail";
pub const PROBE_STATUS: &str = "probe.status";
pub const PROBE_SETTINGS_GET: &str = "probe.settings.get";
pub const PROBE_SETTINGS_SAVE: &str = "probe.settings.save";
pub const PROBE_LOGS_DB_STATUS: &str = "probe.logsDb.status";
pub const PROBE_EVENTS: &str = "probe.events";
pub const PROBE_BARK_TEST: &str = "probe.barkTest";
pub const PROBE_INSTALL_HOOKS: &str = "probe.installHooks";
pub const PROBE_LOGS_DB_DRY_RUN: &str = "probe.logsDbDryRun";
pub const PROBE_LOGS_DB_EXECUTE: &str = "probe.logsDbExecute";
pub const UPDATES_STATUS: &str = "updates.status";
pub const UPDATES_CHECK: &str = "updates.check";
pub const UPDATES_INSTALL: &str = "updates.install";
pub const UPDATES_PRUNE: &str = "updates.prune";
pub const CLEANUP_ARCHIVE_DRY_RUN: &str = "cleanup.archiveDryRun";
pub const CLEANUP_ARCHIVE_EXECUTE: &str = "cleanup.archiveExecute";
pub const CLEANUP_HIDDEN_DRY_RUN: &str = "cleanup.hiddenDryRun";
pub const CLEANUP_HIDDEN_EXECUTE: &str = "cleanup.hiddenExecute";
pub const UPLOADS_DELETE: &str = "uploads.delete";

pub const TRANSPORT_UPLOAD_FILES: &str = "uploadFiles";
pub const TRANSPORT_THREAD_EVENTS: &str = "threadEvents";

pub const ALLOWED_RPC_COMMANDS: &[&str] = &[
    AUTH_PUBLIC_SETTINGS,
    AUTH_LOGIN,
    AUTH_LOGOUT,
    AUTH_ME,
    SECURITY_GET,
    SECURITY_SAVE,
    SECURITY_CHANGE_PASSWORD,
    SYSTEM_STATUS,
    SYSTEM_VERSION,
    SYSTEM_PLATFORM,
    SYSTEM_PROVIDERS,
    SYSTEM_PLUGINS,
    SYSTEM_MODELS,
    SYSTEM_PERMISSION_PROFILES,
    SYSTEM_CODEX_CONFIG,
    SYSTEM_CLAUDE_CODE_OVERVIEW,
    THREADS_LIST,
    THREADS_DETAIL,
    THREADS_BLOCKS,
    THREADS_CREATE,
    THREADS_SEND,
    THREADS_STEER,
    THREADS_STOP,
    THREADS_ARCHIVE,
    THREADS_RESTORE,
    THREADS_RENAME,
    THREADS_FORK,
    THREADS_FOLLOWUPS_LIST,
    THREADS_FOLLOWUPS_ENQUEUE,
    THREADS_FOLLOWUPS_CANCEL,
    THREADS_PLAN_ACCEPT,
    THREADS_PLAN_REVISE,
    THREADS_ELICITATION_ANSWER,
    THREADS_APPROVAL_ANSWER,
    THREADS_GOAL_GET,
    THREADS_GOAL_SAVE,
    THREADS_GOAL_CLEAR,
    THREADS_GOAL_PAUSE,
    THREADS_GOAL_RESUME,
    JOBS_LIST,
    JOBS_DETAIL,
    PROBE_STATUS,
    PROBE_SETTINGS_GET,
    PROBE_SETTINGS_SAVE,
    PROBE_LOGS_DB_STATUS,
    PROBE_EVENTS,
    PROBE_BARK_TEST,
    PROBE_INSTALL_HOOKS,
    PROBE_LOGS_DB_DRY_RUN,
    PROBE_LOGS_DB_EXECUTE,
    UPDATES_STATUS,
    UPDATES_CHECK,
    UPDATES_INSTALL,
    UPDATES_PRUNE,
    CLEANUP_ARCHIVE_DRY_RUN,
    CLEANUP_ARCHIVE_EXECUTE,
    CLEANUP_HIDDEN_DRY_RUN,
    CLEANUP_HIDDEN_EXECUTE,
    UPLOADS_DELETE,
];

pub const ALLOWED_TRANSPORT_COMMANDS: &[&str] = &[TRANSPORT_UPLOAD_FILES, TRANSPORT_THREAD_EVENTS];

pub const RETIRED_COMMANDS: &[&str] = &[
    "getPublicSettings",
    "login",
    "logout",
    "me",
    "getSecurity",
    "saveSecurity",
    "changePassword",
    "listProviders",
    "getClaudeCodeOverview",
    "getPlatformOverview",
    "listPlugins",
    "getProbeStatus",
    "getProbeSettings",
    "saveProbeSettings",
    "getProbeLogsDbStatus",
    "getProbeEvents",
    "startProbeBarkTest",
    "startProbeHooksInstall",
    "startProbeLogsDbDryRun",
    "startProbeLogsDbExecute",
    "startProbeJob",
    "dryRunArchiveDelete",
    "startArchiveDelete",
    "dryRunHiddenThreadDelete",
    "startHiddenThreadDelete",
    "getUpdateStatus",
    "checkUpdate",
    "installUpdateAndRestart",
    "runUpdateAction",
    "backupPrune",
    "listThreads",
    "getThread",
    "getThreadBlocks",
    "createThread",
    "sendMessage",
    "steerThread",
    "listFollowUps",
    "enqueueFollowUp",
    "cancelFollowUp",
    "stopThread",
    "archiveThread",
    "restoreThread",
    "renameThread",
    "forkThread",
    "acceptPlan",
    "revisePlan",
    "answerElicitation",
    "answerApproval",
    "deleteUpload",
    "getSystemStatus",
    "getSystemVersion",
    "listModels",
    "listPermissionProfiles",
    "getCodexConfig",
    "getCodexGoal",
    "saveCodexGoal",
    "clearCodexGoal",
    "pauseCodexGoal",
    "resumeCodexGoal",
    "listJobs",
    "getJob",
    "getDesktopOverview",
    "getDesktopHome",
    "getDesktopPlatformStatus",
    "getDesktopClaudeCodeOverview",
];

pub fn is_allowed_rpc_command(command: &str) -> bool {
    ALLOWED_RPC_COMMANDS.contains(&command)
}

pub fn is_retired_command(command: &str) -> bool {
    RETIRED_COMMANDS.contains(&command)
}

#[cfg(test)]
mod tests {
    use super::{ALLOWED_RPC_COMMANDS, ALLOWED_TRANSPORT_COMMANDS, RETIRED_COMMANDS};
    use std::collections::HashSet;

    #[test]
    fn unified_rpc_commands_are_dot_named_and_unique() {
        let mut seen = HashSet::new();
        for command in ALLOWED_RPC_COMMANDS {
            assert!(
                command.contains('.'),
                "business RPC command must use dot naming: {command}"
            );
            assert!(
                seen.insert(*command),
                "business RPC command must be unique: {command}"
            );
        }
    }

    #[test]
    fn transport_commands_are_explicit_exceptions() {
        assert_eq!(
            ALLOWED_TRANSPORT_COMMANDS,
            &["uploadFiles", "threadEvents"],
            "non-dot command names are reserved for transport endpoints only"
        );
    }

    #[test]
    fn retired_commands_are_not_allowed_rpc_commands() {
        for retired in RETIRED_COMMANDS {
            assert!(
                !ALLOWED_RPC_COMMANDS.contains(retired),
                "retired command must not remain in allowed RPC set: {retired}"
            );
        }
    }
}
