# Module Inventory

| Module | Responsibility | Dependencies | Files | Lines | Complexity | S.U.P.E.R Score |
|:--|:--|:--|--:|--:|:--|:--|
| `app_server` | Historical app-server protocol compatibility types and tests; not a default runtime path | Tokio, serde, app-server protocol | 1 | 1255 | High | Syellow Uyellow Pyellow Eyellow Ryellow |
| `codex` | Codex state DB, rollout parsing, thread/detail model | rusqlite, JSONL, filesystem | 1 | 4119 | Critical | Sred Uyellow Pyellow Eyellow Ryellow |
| `api` | Axum routing, auth gates, Codex actions, jobs, uploads, provider endpoints | `nexushub-core`, Axum | 1 | 4578 | Critical | Sred Uyellow Pyellow Eyellow Ryellow |
| `db` | NexusHub panel DB, sessions, jobs, follow-up queue | rusqlite, crypto | 1 | 1048 | High | Syellow Ugreen Pyellow Egreen Ryellow |
| `archive` | Archive/hidden-thread delete planning and execution | Codex DB/filesystem | 1 | 1156 | High | Syellow Uyellow Pyellow Eyellow Ryellow |
| `uploads` | Upload validation, storage, extraction, prompt context | CSV, ZIP, PDF, DOCX libs | 1 | 779 | Medium | Syellow Ugreen Pyellow Eyellow Rgreen |
| `update` / `jobs` / `system` | Fixed command jobs, failure analysis, system status | shell commands, systemd, GitHub releases | 3 | 1172 | High | Syellow Uyellow Pyellow Eyellow Ryellow |
| `config` / `platform` | Defaults, normalization, three-platform paths | TOML, env, OS cfg | 2 | 516 | Medium | Sgreen Ugreen Pyellow Eyellow Rgreen |
| `providers` / `claude_code` / `probe` | Multi-provider registry, Claude read-only discovery, built-in Probe replacement | filesystem, serde | 3 | 369 | Medium | Sgreen Ugreen Pyellow Eyellow Rgreen |
| `security` / `crypto` / `auth` / `turnstile` | Token hashing, encrypted settings, sessions, Turnstile verification | argon2, AES-GCM, reqwest | 4 | 349 | Medium | Sgreen Ugreen Pyellow Eyellow Ryellow |
| WebUI `App.tsx` | Main UI state, navigation, chat, ops/security/provider pages | React, API client | 1 | 3583 | Critical | Sred Uyellow Pyellow Egreen Ryellow |
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
- **Path**: `crates/nexushub-core/src/codex.rs`
- **Responsibility**: Read official Codex state, rollout data, statuses, messages, plans, questions, approvals, config, models, and goal state.
- **Transformation Notes**: Highest non-regression risk. Refactor only behind tests because it embodies current Codex compatibility.
- **S.U.P.E.R Assessment**: Single-purpose violation due to breadth. Replacement cost is high until thread/message parsing is split behind provider ports.

### `api`
- **Path**: `crates/nexushubd/src/api.rs`
- **Responsibility**: HTTP routing, auth/CSRF enforcement, provider preview endpoints, Codex actions, uploads, jobs, and archive endpoints.
- **Transformation Notes**: Add provider routes around existing Codex endpoints instead of breaking compatibility wrappers.
- **S.U.P.E.R Assessment**: Critical single-purpose hotspot. It is still the main integration point and should be decomposed incrementally.

### WebUI
- **Path**: `webui/src/App.tsx`, `webui/src/lib/`, `webui/src/types.ts`
- **Responsibility**: Dense operations UI, chat/thread workspace, provider preview pages, Probe page, plugins and ops views.
- **Transformation Notes**: Keep current design language. Improve structure by moving page-specific state/components out of `App.tsx` after API contracts stabilize.
- **S.U.P.E.R Assessment**: `App.tsx` is the largest UI hotspot; API/types are more replaceable but still need stronger provider-specific contracts.

### Deploy
- **Path**: `deploy/nexushub/`, `scripts/`, `.github/workflows/`
- **Responsibility**: Linux package/install/update, Nginx snippet, systemd unit, CI/release assets.
- **Transformation Notes**: Linux is real. macOS and Windows are preview/planned until service installers and packaging are verified.
- **S.U.P.E.R Assessment**: Environment assumptions are explicit but platform-specific. Keep path migration tests for old codex-cloud-panel installs.
