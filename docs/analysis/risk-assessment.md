# Risk Assessment

## S.U.P.E.R Architecture Health Summary

| Principle | Status | Key Findings | Transformation Priority |
|:--|:--|:--|:--|
| **S** Single Purpose | yellow | `api.rs`, `codex.rs`, `src-tauri/src/lib.rs`, and `App.tsx` now delegate most behavior to submodules, services, and domain components; `Conversation.tsx` remains the largest UI surface. | Medium |
| **U** Unidirectional Flow | green | Linux RPC and macOS Tauri commands enter shared core use-case/read-model services; frontend components consume query/domain/runtime boundaries. | Medium |
| **P** Ports over Implementation | yellow | Shared DTOs, dot commands, capability policy, and typed Tauri commands are in place; future providers still need provider-specific control ports. | Medium |
| **E** Environment-Agnostic | yellow | Linux and macOS are production acceptance targets; Windows Service remains planned. Linux systemd/Nginx assumptions stay isolated to deploy scripts and capability policy. | Medium |
| **R** Replaceable Parts | yellow | Codex parsing is split into internal modules and adapters are thinner, but provider replacement still depends on Codex-specific read-model tests. | Medium |

**Overall Health**: shared Linux/macOS architecture is in the intended shape for the current Codex-focused product. Remaining risk is mainly around future provider expansion and keeping release acceptance discipline fresh.

### S.U.P.E.R Violation Hotspots

- `webui/src/components/chat/Conversation.tsx`: still large because it composes message stream, current action cards, run config, inspector panels, and composer wiring.
- `crates/nexushub-core/src/codex.rs`: now delegates path/session/thread/rollout/mutation details, but remains the public Codex facade and compatibility anchor.
- `crates/nexushub-webd/src/api.rs`: now mostly router/helper composition; new behavior should still land in `api/*` adapters plus core services rather than growing the entry file.
- `deploy/nexushub-webd/install.sh` and `deploy/nexushub-webd/update.sh`: production-critical script logic with embedded Python migrations and path assumptions.

## Risk Matrix

| Risk | Impact | Likelihood | Severity | Mitigation |
|:--|:--|:--|:--|:--|
| Codex regression while adding providers | Existing panel control breaks | Medium | High | Keep Codex routes and bridge-first flow; add tests before refactors |
| AGPL contamination from external Claude UI references | Licensing conflict | Low | High | Do not copy source/assets/schemas/plugin ABI; document independent implementation |
| App-server exposure | Secret/local control plane exposed | Low | Critical | Only proxy `/nexushub/`; never expose socket, `/v1`, `/responses`, or metrics |
| Archive/hidden-thread delete mistake | Data loss | Medium | High | Preserve dry-run and integrity checks; require button confirmation |
| Config/path migration drift | Broken cloud install/update | Medium | High | Keep install script tests and legacy path fixtures |
| Windows overclaim | Unsupported Windows service packages appear production-ready | Medium | Medium | Keep Windows marked planned until service installers and artifacts are verified |
| Claude Code writes too early | User config/tool permission risk | Medium | High | V1 read-only only; explicit future task required for writes |
| Probe hidden control drift | User loses visible Codex control | Medium | High | Observation/notification first; no hidden desktop recovery in V1 |

## High-Severity Risks

Codex compatibility is the core product risk. The current Codex chain depends on official DB reads, `session_index.jsonl`, rollout parsing, and `logs_2.sqlite` activity. New provider work must wrap this local read model rather than replacing it or reintroducing a root app-server dependency.

Deployment safety is the second core risk. The canonical Linux layout is now `nexushub-webd` with `/usr/local/bin/nexushub-webd`, `/usr/share/nexushub-webd/webui`, `/etc/nexushub-webd`, `/var/lib/nexushub-webd`, and `/var/log/nexushub-webd`. Install/update scripts intentionally keep old `/opt/nexushub` and `codex-cloud-panel` values only as migration inputs, not as current runtime paths.

## Technical Debt

- `Conversation.tsx` can be split further into message stream, inspector, current-action, and run-config components.
- Provider abstractions are present but not yet deep enough to support full Claude/Cursor/Gemini control.
- Windows platform paths exist, but service installers and release assets are not production-grade.
- Provider pages are controlled/read-only surfaces rather than full workspace parity.

## Testing Risks

- Browser-plugin acceptance must keep covering Linux login, Turnstile, security save/reload, Probe, Ops, cleanup dry-run gating, and scoped sensitive-path `404`.
- Computer Use acceptance must keep covering official macOS DMG install, visible non-fullscreen Tauri window, App/helper exact version, and absence of Web auth/Linux-only surfaces.
- Windows Service flows need future dry-run tests before any production claim.
- `docs/progress/MASTER.md` is the current release/deploy evidence source; old `v0.1.43` phase notes are historical snapshots only.

## Project Governance Risks

The repo started without `AGENTS.md` or `docs/progress/MASTER.md`. Those surfaces now exist. `CLAUDE.md` is intentionally absent because the user deleted it, and agents must not restore it unless the user explicitly requests that file. Durable memory remains native; no repo-local memory fallback has been selected by the user.

## Compatibility Concerns

- Existing codex-cloud-panel browser sessions are not preserved after renaming the cookie to `nexushub_session`.
- Legacy install config paths are migrated only when they match known old values.
- `/home/ubuntu/codex-admin/bin/codex-cloud-*` wrapper names intentionally remain because they belong to the existing cloud Codex chain.
