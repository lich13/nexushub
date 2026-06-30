# NexusHub Feature Sync Workflow

Last reviewed: 2026-06-30  
Applies from: `v0.1.145`

This is the default flow for adding or changing shared NexusHub behavior across macOS Tauri, Linux Tauri, Tencent Cloud Linux WebUI, and desktop LAN WebUI.

## 1. Contract First

Run the checklist before editing implementation:

```bash
node scripts/contract-next-action-checklist.mjs <action-id>
```

For a new action, first update `contracts/nexushub-contract.json` with:

- action id, `kind`, `scope`, and `coreUseCase`
- `linuxRpc`, `tauriCommand`, and `webuiWrapper` for `scope=shared`
- `hostOnlyReason` for `scope=host_only`
- `dtoOwner`, `requestDto`, and `responseDto` for `scope=shared` or `scope=transport`
- capability or visual rules when the feature changes what a host surface can see

Keep `contracts/nexushub-contract.schema.json` in sync.

## 2. Core Use-Case And DTO

Shared behavior enters through `nexushub_core::services::use_cases::NexusHubUseCases`.

Update the owning core service, request/response DTO, and plan types before touching adapters. Add the DTO names to `crates/nexushub-core/src/services/contract_dtos.rs` so Rust guards catch registry drift.

## 3. WebUI Query/Domain/Runtime

Update the shared `webui` layer:

- API wrapper under `webui/src/lib/api/`
- domain or view-model helper under `webui/src/lib/domain/`
- runtime transport policy only when the host surface differs
- `webui/src/lib/domain/contractDtoMap.ts` for DTO marker coverage
- visual/capability tests when navigation, copy, disabled state, or host visibility changes

Components should consume hooks, domain helpers, and capability props. They should not import raw transport, raw Tauri invoke, or React Query cache primitives.

## 4. Thin Adapters

Add the Linux RPC dispatcher mapping and the typed Tauri command mapping last.

Linux WebUI must keep the public wire shape under `/api/rpc/:command`. Tauri commands should call the same shared plans and keep native effects narrow. Host-only commands stay behind capability and host-surface policy.

## 5. Verification And Acceptance

Run the relevant local guards first:

```bash
cargo test --workspace
corepack pnpm@11.0.8 --dir webui test
bash scripts/test-install-script.sh
```

For visible behavior, finish with the matching现场验收:

- Browser 插件: Linux WebUI login, Turnstile, security save/reload, Probe, Ops, cleanup dry-run gating, scoped sensitive paths `404`
- Computer Use: macOS Tauri shared pages, helper version, desktop `WebUI 服务`, no Linux-only surface leakage, desktop LAN WebUI when touched

Do not treat a shared feature as complete until both the contract parity guards and the affected host-surface behavior pass fresh verification.
