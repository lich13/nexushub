# Module Inventory

| Module | Responsibility | Dependencies | Files | Lines | Complexity | S.U.P.E.R Score |
|:--|:--|:--|--:|--:|:--|:--|
| `app_server` | Historical app-server protocol compatibility types and tests; not a default runtime path | Tokio, serde, app-server protocol | 1 | 1255 | High | Syellow Uyellow Pyellow Eyellow Ryellow |
| `codex` | Public Codex facade plus split path/session/thread/rollout/mutation read-model modules | rusqlite, JSONL, filesystem | 7 | 3025 facade lines plus submodules | High | Syellow Ugreen Pyellow Eyellow Ryellow |
| `api` | Axum router/helpers plus thin domain adapters under `api/*` | `nexushub-core`, Axum | 12 | 3433 entry lines plus adapters | High | Syellow Ugreen Pyellow Eyellow Ryellow |
| `db` | NexusHub panel DB, sessions, jobs, follow-up queue | rusqlite, crypto | 1 | 1048 | High | Syellow Ugreen Pyellow Egreen Ryellow |
| `archive` | Archive/hidden-thread delete planning and execution | Codex DB/filesystem | 1 | 1156 | High | Syellow Uyellow Pyellow Eyellow Ryellow |
| `uploads` | Upload validation, storage, extraction, prompt context | CSV, ZIP, PDF, DOCX libs | 1 | 779 | Medium | Syellow Ugreen Pyellow Eyellow Rgreen |
| `update` / `jobs` / `system` | Fixed command jobs, failure analysis, system status | shell commands, systemd, GitHub releases | 3 | 1172 | High | Syellow Uyellow Pyellow Eyellow Ryellow |
| `config` / `platform` | Defaults, normalization, three-platform paths | TOML, env, OS cfg | 2 | 516 | Medium | Sgreen Ugreen Pyellow Eyellow Rgreen |
| `providers` / `claude_code` / `probe` | Multi-provider registry, Claude read-only discovery, built-in Probe replacement | filesystem, serde | 3 | 369 | Medium | Sgreen Ugreen Pyellow Eyellow Rgreen |
| `security` / `crypto` / `auth` / `turnstile` | Token hashing, encrypted settings, sessions, Turnstile verification | argon2, AES-GCM, reqwest | 4 | 349 | Medium | Sgreen Ugreen Pyellow Eyellow Ryellow |
| WebUI `App.tsx` / components / domain view-models | Shell composition, chat/security/ops/probe workspaces, composer and conversation domain view-models | React, query hooks, capability policy | 10+ | `App.tsx` 483, `Conversation.tsx` 1574, `ComposerControls.tsx` 358 | Medium | Syellow Ugreen Pyellow Egreen Ryellow |
| WebUI `api.ts` / types / stores | API contracts, demo fallbacks, message cache | fetch, Vitest | 5 | 3342 | High | Syellow Ugreen Pyellow Egreen Ryellow |
| Deploy scripts | Install, update, wrappers, package, cloud runbook | bash, Python snippets, systemd/nginx | 8 | n/a | High | Syellow Uyellow Pyellow Ered Ryellow |

## Module Details

### `app_server`
- **Path**: `crates/nexushub-core/src/app_server.rs`
- **Responsibility**: Preserve historical app-server protocol compatibility surfaces while the default runtime uses local state and controlled jobs.
- **Public API**: `AppServerBridge`, `BridgeActionResult`, `BridgeTurnOptions`.
- **Transformation Notes**: Do not reintroduce this module as a default runtime dependency; any future use must stay explicitly configured and private.
- **S.U.P.E.R Assessment**: Single purpose is partial because protocol details and retry/error shaping live together. Ports need stronger typed contracts before more providers reuse the shape.

### `codex`
- **Path**: `crates/nexushub-core/src/codex.rs`, `crates/nexushub-core/src/codex/*`
- **Responsibility**: Preserve the public Codex facade while delegating path discovery, session index, thread DB rows, rollout events, DTO/types, mutations, and integrity checks to internal modules.
- **Transformation Notes**: Highest non-regression risk. Keep public `nexushub_core::codex::*` compatibility and add tests before behavior changes.
- **S.U.P.E.R Assessment**: Improved single-purpose and unidirectional flow, but still the compatibility anchor for current Codex behavior.

### `api`
- **Path**: `crates/nexushub-webd/src/api.rs`, `crates/nexushub-webd/src/api/*`
- **Responsibility**: Compose Axum routes, public response helpers, RPC dispatch, auth/CSRF gates, and thin adapters for cleanup, goals, jobs, Probe, security, system, threads, uploads, and Web auth.
- **Transformation Notes**: Keep `/api/rpc/:command` stable; place business contracts in core services and adapter-specific effects in submodules.
- **S.U.P.E.R Assessment**: Entry file is no longer the business owner, but router growth and static guards must stay monitored.

### WebUI
- **Path**: `webui/src/App.tsx`, `webui/src/components/*`, `webui/src/lib/domain/*`, `webui/src/lib/query/*`, `webui/src/types.ts`
- **Responsibility**: Keep `App.tsx` as runtime/session/navigation composition; components render domain workspaces; query hooks own network/cache effects; domain view-models own pure conversation/composer/runtime derivations.
- **Transformation Notes**: Continue splitting `Conversation.tsx` by UI section if it grows again. Linux/macOS differences must remain capability-matrix and host-policy driven.
- **S.U.P.E.R Assessment**: `App.tsx` is no longer the largest hotspot; conversation composition remains medium risk.

### Deploy
- **Path**: `deploy/nexushub-webd/`, `deploy/desktop/`, `scripts/`, `.github/workflows/`
- **Responsibility**: Linux `nexushub-webd` package/install/update, Nginx snippet, systemd unit, Tauri desktop packaging, CI/release assets.
- **Transformation Notes**: Linux server WebUI, macOS Tauri, and Linux Tauri are release acceptance targets. Windows remains planned until service installers and artifacts are verified.
- **S.U.P.E.R Assessment**: Environment assumptions are explicit but platform-specific. Keep path migration tests for old codex-cloud-panel installs.
