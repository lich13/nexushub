# Module Inventory

| Module | Current responsibility |
| --- | --- |
| `codex` | Local state/title discovery, task identity, rollout parsing, status and official restricted Goal client |
| `grok` | Native session history, grouped tool updates, narrow rename protocol and confirmed local-directory deletion |
| `read_cache` | Bounded file-snapshot cache with inode/mtime/ctime invalidation |
| `probe` / error monitor | Main-task notification selection, dedupe, redaction, logs cursor and restricted recovery |
| `services/use_cases` | Shared browse, rename/archive, cleanup, Probe, security and update contracts |
| `jobs` / `update` | Fixed maintenance/update jobs and redacted execution history |
| `archive` | Dry-run and count-checked archive/hidden cleanup |
| `db` | Settings, events, incidents, sessions, historical jobs/followups and compatibility data |
| `config` / `platform` | Resolved provider/runtime paths and persistent settings |
| HTTP `api/*` | Auth/CSRF, contract allowlist and thin Linux adapters |
| Tauri `services/*` | Embedded desktop adapters, helper synchronization and monitor lifecycle |
| WebUI components/query/domain | Compact reading interface, data/cache effects and pure view models |
| Deploy/scripts/workflows | Exact-tag packaging, signed updater metadata, install/health and release gates |

The retired upload, Claude and Codex task-execution modules are not compatibility APIs. Do not restore them to support historical DB rows. The current contract registry, not historical progress matrices or source line counts, defines public behavior.
