export type ThreadStatus = "Recent" | "Running" | "ReplyNeeded" | "Recoverable" | "Archived";

export type SessionUser = {
  id?: string;
  username: string;
  csrf_token?: string | null;
  session_id?: string;
};

export type PublicSettings = {
  site_name: string;
  turnstile_enabled: boolean;
  turnstile_required?: boolean;
  turnstile_site_key: string;
  turnstile_action?: string | null;
  admin_configured: boolean;
};

export type ThreadSummary = {
  id: string;
  title: string;
  status: ThreadStatus;
  updated_at?: string | null;
  archived_at?: string | null;
  message_count: number;
  latest_message?: string | null;
  cwd?: string | null;
  model?: string | null;
  rollout_path?: string | null;
  active_turn_id?: string | null;
  active_job_id?: string | null;
  pending_elicitation?: PendingElicitation | null;
  last_event_kind?: string | null;
  thread_source?: string | null;
  threadSource?: string | null;
  source_kind?: string | null;
  sourceKind?: string | null;
  parent_thread_id?: string | null;
  parentThreadId?: string | null;
  source?: unknown;
  agent_nickname?: string | null;
  agentNickname?: string | null;
  agent_role?: string | null;
  agentRole?: string | null;
  agent_path?: string | null;
  agentPath?: string | null;
  has_user_event?: number | boolean | null;
  hasUserEvent?: number | boolean | null;
  first_user_message?: string | null;
  firstUserMessage?: string | null;
  preview?: string | null;
};

export type CodexMessage = {
  role: string;
  kind: string;
  text: string;
  created_at?: string | null;
};

export type UserInputOption = {
  label: string;
  description?: string | null;
};

export type UserInputQuestion = {
  id: string;
  header?: string | null;
  question: string;
  options: UserInputOption[];
};

export type PendingElicitation = {
  turn_id?: string | null;
  item_id?: string | null;
  questions: UserInputQuestion[];
};

export type MessageBlock = {
  id: string;
  role: string;
  kind: string;
  display_kind?: string | null;
  status?: string | null;
  text?: string | null;
  summary?: string | null;
  input?: string | null;
  truncated?: boolean | null;
  resolved?: boolean | null;
  answers?: UserInputAnswer[];
  plan_status?: string | null;
  group_id?: string | null;
  tool_name?: string | null;
  call_id?: string | null;
  turn_id?: string | null;
  item_id?: string | null;
  created_at?: string | null;
  questions: UserInputQuestion[];
  payload?: unknown;
};

export type UserInputAnswer = {
  question_id: string;
  answers: string[];
  note?: string | null;
};

export type ThreadDetail = {
  summary: ThreadSummary;
  messages: CodexMessage[];
  blocks: MessageBlock[];
  raw_event_count: number;
  total_blocks?: number;
  has_more_blocks?: boolean;
  before_cursor?: string | null;
};

export type ThreadBlockPage = {
  thread_id: string;
  blocks: MessageBlock[];
  total_blocks: number;
  has_more_blocks: boolean;
  before_cursor?: string | null;
};

export type BridgeActionResult = {
  bridge: boolean;
  thread_id?: string | null;
  turn_id?: string | null;
  job_id?: string | null;
  fallback: boolean;
  message?: string | null;
};

export type UploadKind = "text" | "markdown" | "spreadsheet" | "document" | "pdf" | "image" | "file";

export type UploadRecord = {
  id: string;
  name: string;
  mime: string;
  size: number;
  sha256: string;
  kind: UploadKind;
  status: string;
  error_preview?: string | null;
};

export type UploadOutcome = {
  files: UploadRecord[];
};

export type FollowUpQueueItem = {
  id: string;
  thread_id: string;
  status: "pending" | "submitting" | "submitted" | "cancelled" | "error" | string;
  message: string;
  options?: unknown;
  created_at?: number | null;
  updated_at?: number | null;
  submitted_at?: number | null;
  cancelled_at?: number | null;
  result?: unknown;
  error?: string | null;
};

export type FollowUpQueueState = {
  items: FollowUpQueueItem[];
};

export type SystemStatus = {
  host_label: string;
  hostname?: string | null;
  public_endpoint?: string | null;
  codex_home: string;
  configured_codex_home?: string | null;
  resolved_codex_home?: string | null;
  codex_home_source?: string | null;
  logs_db_source?: string | null;
  discovery_warnings?: string[] | null;
  state_db?: string | null;
  panel_db: string;
  state_db_integrity?: string | null;
  hidden_thread_count?: number | null;
  thread_source_counts?: Record<string, number> | null;
};

export type SystemVersion = {
  panel_current: string;
  panel_latest?: string | null;
  panel_update_available?: boolean | null;
  codex_current?: string | null;
  codex_latest?: string | null;
  codex_update_available?: boolean | null;
  codex_user?: string | null;
  codex_root?: string | null;
  codex_raw?: string | null;
};

export type SecuritySettings = {
  turnstile_enabled: boolean;
  turnstile_required: boolean;
  turnstile_site_key: string;
  turnstile_secret_configured: boolean;
  session_ttl_seconds: number;
  turnstile_expected_hostname?: string | null;
  turnstile_expected_action?: string | null;
};

export type JobRecord = {
  id: string;
  kind: string;
  status: string;
  title: string;
  thread_id?: string | null;
  turn_id?: string | null;
  started_at: number;
  finished_at?: number | null;
  exit_code?: number | null;
  output: string;
  error?: string | null;
  analysis?: string | null;
  explanation?: string | null;
  failure_analysis?: {
    category: string;
    explanation: string;
    suggestions: string[];
  } | null;
};

export type ArchiveDeletePlan = {
  total_threads: number;
  active_threads: number;
  archived_threads: number;
  session_index_lines: number;
  rollout_files: number;
  archived_ids: string[];
  integrity: string;
};

export type HiddenThreadDeletePlan = {
  total_threads: number;
  visible_threads: number;
  hidden_threads: number;
  archived_threads: number;
  session_index_lines: number;
  rollout_files: number;
  hidden_ids: string[];
  hidden_source_counts: Record<string, number>;
  integrity: string;
};

export type HiddenThreadDeleteResult = {
  before: HiddenThreadDeletePlan;
  deleted_threads: number;
  after_total_threads: number;
  after_visible_threads: number;
  after_hidden_threads: number;
  after_archived_threads: number;
  after_integrity: string;
  visible_threads: number;
  hidden_threads: number;
  integrity: string;
  deleted_rollout_files: number;
};

export type OptionalResult<T> = {
  available: boolean;
  data?: T;
  error?: string;
  reason?: string | null;
};

export type AgentProviderInfo = {
  id: "codex" | "claude_code" | "cursor" | "gemini" | string;
  label: string;
  status: "ready" | "preview" | "planned" | string;
  description?: string;
  capabilities?: string[];
  safety?: string;
};

export type ClaudeSessionSummary = {
  id: string;
  title?: string | null;
  updated_at?: string | null;
  message_count: number;
  file?: string | null;
  last_message_preview?: string | null;
};

export type ClaudeProject = {
  id: string;
  display_name: string;
  path_hint?: string | null;
  session_count: number;
  sessions: ClaudeSessionSummary[];
};

export type ClaudeRecentSession = ClaudeSessionSummary & {
  project_id: string;
  project_display_name: string;
};

export type ClaudeMcpSummary = {
  config_files: string[];
  server_count: number;
  servers: Array<{
    name: string;
    command?: string | null;
    transport?: string | null;
    args_count: number;
    env_keys: string[];
    has_sensitive_env: boolean;
    raw_config?: unknown;
  }>;
};

export type ClaudeInstallationSummary = {
  claude_home: string;
  settings_file: string;
  settings_exists: boolean;
  settings_local_file: string;
  settings_local_exists: boolean;
  user_config_file?: string | null;
  user_config_exists: boolean;
  executable_candidates: string[];
  version_hint?: string | null;
  health_hints: string[];
};

export type ClaudeCacheLogStatus = {
  cache_dir: string;
  cache_exists: boolean;
  cache_file_count: number;
  cache_total_bytes: number;
  log_dir: string;
  log_exists: boolean;
  log_file_count: number;
  log_total_bytes: number;
};

export type ClaudeMaintenanceCommand = {
  name: string;
  title: string;
  command: string;
  description: string;
};

export type ClaudeOverview = {
  home: string;
  settings_exists: boolean;
  settings_preview?: unknown;
  projects: ClaudeProject[];
  recent_sessions?: ClaudeRecentSession[];
  mcp?: ClaudeMcpSummary;
  installation?: ClaudeInstallationSummary;
  cache_status?: ClaudeCacheLogStatus;
  maintenance_commands?: Record<string, ClaudeMaintenanceCommand>;
};

export type PlatformOverview = {
  kind: "linux" | "macos" | "windows" | string;
  data_dir: string;
  config_file: string;
  webui_dir: string;
  log_dir: string;
  service_name: string;
  service_kind: string;
};

export type PluginInfo = {
  id: string;
  label: string;
  status: "ready" | "preview" | "planned" | string;
  kind: "builtin" | "external" | string;
  description?: string | null;
  unavailable_reason?: string | null;
  invocation_template?: string | null;
};

export type ProbeStatus = {
  label?: string | null;
  enabled: boolean;
  available?: boolean | null;
  platform: "linux" | "macos" | "windows" | string;
  service_kind: string;
  service_name: string;
  flavor?: string | null;
  hook_status: string;
  bark_status: string;
  logs_db_status: string;
  recent_event_count: number;
  running_count: number;
  reply_needed_count: number;
  recoverable_count: number;
  running_threads?: ThreadSummary[];
  reply_needed_threads?: ThreadSummary[];
  recoverable_threads?: ThreadSummary[];
  config_path: string;
  lifecycle_status?: string | null;
  doctor_status?: string | null;
  runtime_version?: string | null;
  codex_home?: string | null;
  configured_codex_home?: string | null;
  resolved_codex_home?: string | null;
  codex_home_source?: string | null;
  logs_db_source?: string | null;
  discovery_warnings?: string[] | null;
  host_label?: string | null;
  snapshot_age_seconds?: number | null;
  is_refreshing?: boolean | null;
  snapshot_status?: string | null;
};

export type ProbeEvent = {
  id: string;
  kind: string;
  thread_id?: string | null;
  title?: string | null;
  message?: string | null;
  dedupe_key?: string | null;
  source: string;
  payload: Record<string, unknown>;
  created_at: string | number;
  handled_at?: string | number | null;
};

export type ProbeEventsResponse = {
  events: ProbeEvent[];
  limit?: number | null;
};

export type ProbeJobAction = "bark-test" | "hooks-install" | "logs-db-dry-run" | "logs-db-execute";

export type ProbeSettings = {
  codex: {
    home?: string | null;
    configured_codex_home?: string | null;
    resolved_codex_home?: string | null;
    codex_home_source?: string | null;
    logs_db_source?: string | null;
    discovery_warnings?: string[] | null;
    workspace?: string | null;
    host_label: string;
  };
  probe: Record<string, unknown> & {
    enabled?: boolean;
    poll_seconds?: number;
    recent_limit?: number;
    hooks?: Record<string, unknown> & {
      manage_stop_hook?: boolean;
    };
    notifications?: Record<string, unknown> & {
      enabled?: boolean;
      server_url?: string;
      sound?: string | null;
      group?: string;
      url?: string | null;
      notify_completion?: boolean;
      notify_reply_needed?: boolean;
      notify_recoverable?: boolean;
    };
    observability?: Record<string, unknown> & {
      hook_event_max_lines?: number;
      hook_cooldown_max_lines?: number;
      log_max_bytes?: number;
    };
    logs_db?: Record<string, unknown> & {
      enabled?: boolean;
      retention_days?: number;
      maintenance_interval_hours?: number;
      maintain_on_codex_exit?: boolean;
      codex_exit_grace_seconds?: number;
      codex_exit_max_wait_seconds?: number;
      delete_chunk_rows?: number;
      max_delete_rows_per_run?: number;
      busy_timeout_ms?: number;
      auto_compact_when_codex_closed?: boolean;
      compact_interval_hours?: number;
      compact_min_freelist_mb?: number;
      compact_min_freelist_ratio_percent?: number;
      minimum_free_space_mb?: number;
    };
  };
  discovery_warnings?: string[] | null;
  notifications: Record<string, unknown> & {
    enabled?: boolean;
    device_key?: string;
    device_key_configured?: boolean;
    server_url?: string;
    sound?: string | null;
    group?: string;
    url?: string | null;
    notify_completion?: boolean;
    notify_reply_needed?: boolean;
    notify_recoverable?: boolean;
  };
  logs_db: Record<string, unknown> & {
    path?: string | null;
    resolved_path?: string | null;
    resolved_logs_db_path?: string | null;
    logs_db_path?: string | null;
    source?: string | null;
    logs_db_source?: string | null;
    enabled?: boolean;
    retention_days?: number;
    maintenance_interval_hours?: number;
    maintain_on_codex_exit?: boolean;
    codex_exit_grace_seconds?: number;
    codex_exit_max_wait_seconds?: number;
    delete_chunk_rows?: number;
    max_delete_rows_per_run?: number;
    busy_timeout_ms?: number;
    auto_compact_when_codex_closed?: boolean;
    compact_interval_hours?: number;
    compact_min_freelist_mb?: number;
    compact_min_freelist_ratio_percent?: number;
    minimum_free_space_mb?: number;
  };
};

export type ProbeLogsDbStatus = {
  status?: string | null;
  logs_db_status?: string | null;
  target?: string | null;
  path?: string | null;
  configured_codex_home?: string | null;
  resolved_codex_home?: string | null;
  codex_home_source?: string | null;
  discovery_warnings?: string[] | null;
  resolved_path?: string | null;
  resolved_logs_db_path?: string | null;
  logs_db_path?: string | null;
  source?: string | null;
  logs_db_source?: string | null;
  size_bytes?: number | null;
  db_size_bytes?: number | null;
  database_size_bytes?: number | null;
  database_size?: number | null;
  wal_size_bytes?: number | null;
  wal_bytes?: number | null;
  wal_size?: number | null;
  shm_size_bytes?: number | null;
  shm_bytes?: number | null;
  shm_size?: number | null;
  old_rows?: number | null;
  retained_rows?: number | null;
  retained_row_count?: number | null;
  total_rows?: number | null;
  row_count?: number | null;
  event_count?: number | null;
  pending_cleanup_rows?: number | null;
  stale_rows?: number | null;
  would_delete_probe_events?: number | null;
  last_run_at?: string | number | null;
  last_maintain_at?: string | number | null;
  last_maintenance_at?: string | number | null;
  last_maintain?: string | number | null;
  next_run_at?: string | number | null;
  next_maintain_at?: string | number | null;
  next_maintenance_at?: string | number | null;
  recent_result?: string | number | boolean | null;
  last_result?: string | number | boolean | null;
  last_maintain_result?: string | number | boolean | null;
  last_run?: unknown;
  skip_reason?: string | null;
  [key: string]: unknown;
};

export type SentinelStatus = ProbeStatus;

export type ClaudeCodeJobAction = "version-check" | "update-precheck" | "update-start" | "smoke" | "cache-status";

export type CodexModel = {
  id: string;
  label?: string | null;
  description?: string | null;
  default?: boolean | null;
  service_tiers?: Array<{ id: string; name?: string | null; description?: string | null }>;
  default_service_tier?: string | null;
};

export type PermissionProfile = {
  id: string;
  label?: string | null;
  description?: string | null;
  approval_policy?: string | null;
  sandbox_mode?: string | null;
  network_access?: boolean | null;
  default?: boolean | null;
};

export type CodexConfig = {
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

export type CodexGoalStatus = "idle" | "active" | "paused" | "cleared" | "blocked" | "complete" | "completed" | "missing_thread" | "unavailable" | string;

export type CodexGoal = {
  available: boolean;
  enabled: boolean;
  objective: string | null;
  token_budget: number | null;
  status: CodexGoalStatus;
  completed_at?: number | null;
  blocked_reason?: string | null;
  raw?: unknown;
};

export type CodexGoalSaveInput = {
  objective: string;
  token_budget?: number | null;
};
