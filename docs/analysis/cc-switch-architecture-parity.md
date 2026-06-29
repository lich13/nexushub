# cc-switch Architecture Parity Audit

Last reviewed: 2026-06-29  
NexusHub target: `v0.1.144`

## Boundary

`cc-switch origin/main` is the public reference for the desktop Tauri release model. It has a wider desktop release matrix than NexusHub, including Windows and Linux arm64, and writes updater metadata for those desktop platforms.

The local `cc-switch feat/webd` branch is a separate reference for `webd`, FHS paths, headless WebUI packaging, and server deployment ideas. It is not the same as `cc-switch origin/main`, and it should not be copied into NexusHub as a dispatcher style when NexusHub already has a stricter shared core/use-case/contract/adapter split.

NexusHub v0.1.144 keeps the accepted product target: macOS Tauri, Linux Tauri x86_64, Tencent Cloud Linux headless WebUI, and desktop LAN WebUI all share one `webui` and the same contract registry.

## Must-Have Parity

- macOS Tauri wraps the shared `webui` directly, with official DMG/updater assets and Computer Use acceptance.
- Tencent Cloud Linux uses `nexushub-webd-linux-x86_64.tar.gz` for headless WebUI/systemd deployment and Browser 插件验收.
- Linux desktop uses `NexusHub-*-Linux-x86_64.AppImage`, `.deb`, and `.rpm`; GitHub Actions `xvfb` smoke is the GUI acceptance path.
- Shared features must start in `contracts/nexushub-contract.json`, then core use-case/DTO, WebUI query/domain/runtime, and finally thin Linux RPC plus Tauri invoke adapters.

## NexusHub Is Intentionally Stricter

- `contracts/nexushub-contract.json` is the single parity registry for shared action ids, host surfaces, capabilities, visual rules, Linux RPC exposure, Tauri invoke exposure, and WebUI wrappers.
- Linux server WebUI, desktop embedded Tauri, and desktop LAN WebUI are separate host surfaces; differences must come from the registry, `SystemCapabilities`, host policy, or runtime transport.
- Browser clients of desktop LAN WebUI cannot start or stop the LAN service, and do not see Linux server systemd, Nginx, public endpoint, security admin, or prune surfaces.
- The headless server tarball is never a Tauri updater platform and must not appear in `latest.json`.

## Intentional Differences

- Windows desktop is a `cc-switch origin/main` capability but remains P2 for NexusHub because the current acceptance target is macOS local and Tencent Cloud Linux.
- Linux arm64 desktop/headless packages remain P2. Adding them would require new GitHub runner coverage, asset guards, updater metadata, and acceptance evidence.
- NexusHub keeps two Linux release lines because they have different duties: `nexushub-webd-linux-x86_64.tar.gz` for Tencent Cloud headless WebUI/systemd, and `NexusHub-*-Linux-x86_64.AppImage` plus `.deb`/`.rpm` for Linux Tauri desktop.
- Linux Tauri builds are expected to be slower than headless webd builds because they install WebKit/GTK dependencies, build Tauri bundles, package AppImage/deb/rpm, sign updater assets, and run `xvfb`.

## Drift Guards

- WebUI tests load `contracts/nexushub-contract.json` directly and verify visual/capability copy, WebUI wrappers, and host surface labels.
- Rust core tests compare host surfaces, capabilities, Linux RPC/transport commands, and shared/host-only action metadata with the contract registry.
- Tauri guard tests compare the registered invoke handler against contract `tauriCommand` entries.
- Linux RPC guard tests compare dispatcher match arms against contract `linuxRpc` entries.
- Release guards verify the exact supported asset list and ensure `latest.json` references only Tauri updater platforms.
