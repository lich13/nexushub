# Risk Assessment

## S.U.P.E.R Architecture Health Summary

| Principle | Status | Key Findings | Transformation Priority |
|:--|:--|:--|:--|
| **S** Single Purpose | red | `api.rs`, `codex.rs`, and `App.tsx` carry many responsibilities. | High |
| **U** Unidirectional Flow | yellow | Backend mostly flows API -> core, but UI and API orchestration are broad. | Medium |
| **P** Ports over Implementation | yellow | New provider registry exists, but many Codex contracts are still route-specific rather than provider-port contracts. | High |
| **E** Environment-Agnostic | yellow | Linux paths are production-ready; macOS/Windows are path previews; deploy scripts assume systemd/Nginx. | Medium |
| **R** Replaceable Parts | yellow | Codex cannot yet be swapped independently because parsing, control, and UI state are intertwined. | High |

**Overall Health**: 0/5 fully healthy - Refactoring Needed. The project is shippable as a conservative Linux/Codex-first V1, but the provider architecture needs gradual boundary hardening before broad multi-CLI control.

### S.U.P.E.R Violation Hotspots

- `crates/nexushubd/src/api.rs`: routing, business logic, auth gates, fallback behavior, uploads, jobs, and provider endpoints in one file.
- `crates/nexushub-core/src/codex.rs`: DB access, rollout parsing, message normalization, status detection, and config/model behavior in one file.
- `webui/src/App.tsx`: navigation, thread state, chat rendering, provider preview pages, settings, and operations pages in one component tree.
- `deploy/nexushub/install.sh` and `deploy/nexushub/update.sh`: production-critical script logic with embedded Python migrations and path assumptions.

## Risk Matrix

| Risk | Impact | Likelihood | Severity | Mitigation |
|:--|:--|:--|:--|:--|
| Codex regression while adding providers | Existing panel control breaks | Medium | High | Keep Codex routes and bridge-first flow; add tests before refactors |
| AGPL contamination from external Claude UI references | Licensing conflict | Low | High | Do not copy source/assets/schemas/plugin ABI; document independent implementation |
| App-server exposure | Secret/local control plane exposed | Low | Critical | Only proxy `/nexushub/`; never expose socket, `/v1`, `/responses`, or metrics |
| Archive/hidden-thread delete mistake | Data loss | Medium | High | Preserve dry-run and integrity checks; require button confirmation |
| Config/path migration drift | Broken cloud install/update | Medium | High | Keep install script tests and legacy path fixtures |
| macOS/Windows overclaim | Unsupported packages appear production-ready | Medium | Medium | Mark as preview/planned until service installers and artifacts are verified |
| Claude Code writes too early | User config/tool permission risk | Medium | High | V1 read-only only; explicit future task required for writes |
| Probe hidden control drift | User loses visible Codex control | Medium | High | Observation/notification first; no hidden desktop recovery in V1 |

## High-Severity Risks

Codex compatibility is the core product risk. The current Codex chain depends on official DB reads, rollout parsing, and app-server bridge behavior. New provider work must wrap this behavior rather than replacing it.

Deployment safety is the second core risk. `/opt/nexushub` is now the canonical Linux layout, while install/update scripts intentionally keep legacy `codex-cloud-panel` replacement tables. Those legacy strings are compatibility inputs, not stale branding.

## Technical Debt

- Large files make targeted reviews harder.
- Provider abstractions are present but not yet deep enough to support full Claude/Cursor/Gemini control.
- Platform paths exist for macOS and Windows, but installers and release assets are not production-grade.
- WebUI provider pages are preview surfaces rather than full workspace parity.

## Testing Risks

- No end-to-end browser test currently proves the provider navigation and mobile layout in a real browser.
- Live bridge behavior is hard to test without the cloud app-server.
- macOS launchd and Windows Service flows need future dry-run tests.
- Linux release/deploy validation has been run for `v0.1.43` on `43.155.235.227` under `/nexushub/`; keep repeating that verification for every release.

## Project Governance Risks

The repo started without `AGENTS.md`, `CLAUDE.md`, or `docs/progress/MASTER.md`. This pass establishes those surfaces. Durable memory remains native; no repo-local memory fallback has been selected by the user.

## Compatibility Concerns

- Existing codex-cloud-panel browser sessions are not preserved after renaming the cookie to `nexushub_session`.
- Legacy install config paths are migrated only when they match known old values.
- `/home/ubuntu/codex-admin/bin/codex-cloud-*` wrapper names intentionally remain because they belong to the existing cloud Codex chain.
