# Current status

- Version: `1.0.0`.
- Scope: three-provider batch management, Grok/Pi Bark extensions, Codex log-maintenance retirement, privacy cleanup and one-time Git history rebuild.
- Current Markdown set: `README.md`, `AGENTS.md`, `DESIGN.md`, `docs/ARCHITECTURE.md`, `docs/cloud-deploy-runbook.md` and this file.

## Implementation

- Parentless release root: `d5e0542f92b1dcc8d35b2fe64f78dd3a88d08ae6`.
- Runtime cleanup migration: `c5611cbd4851aac2ef1c9fbf94ddc4a0ed0c85fd`.
- Released implementation: `37bcc3591d8300d7b97ce238a45810d145ab396a`; `v1.0.0` points to this commit. Later evidence updates are normal incremental commits.
- The migration removes only retired Codex log-maintenance jobs and state keys; provider sessions, configuration, secrets and unrelated jobs remain.
- Batch actions, native Pi JSONL support, provider-specific Bark completion rules, privacy scanning and the systemd session write allowlist are in the release tree.
- The Tauri thread service now passes the requested archive filter into the Codex reader, so the archive tab includes archived threads in the installed App as well as in the core read model.

## Release evidence

- `v1.0.0` has 15 assets, verified sidecar hashes, updater signatures and Darwin/Linux updater mappings.
- The implementation's four-job [CI](https://github.com/lich13/nexushub/actions/runs/35895662480) and [Release](https://github.com/lich13/nexushub/actions/runs/35896634567) passed at that exact SHA, including the Linux AppImage xvfb smoke.
- The official macOS App and bundled/installed helpers report `1.0.0`; helper hashes match. The App launches and the Pi batch preview protects an active or uncertain session. The monitor remains active with zero TCP listeners after App closure. The executable has an ad-hoc signature; the bundle is unsealed and strict bundle verification fails. Developer ID signing and notarization are not claimed. Both updater package signatures passed verification.
- The authenticated cloud WebUI reports `1.0.0`, health is active, the writable Grok session exception is present, and missing Pi storage remains empty without installation. Batch selection, precise deletion preview, ID copy, provider Bark settings and retired-route 404s were verified. No live user session was deleted during this acceptance.
- The final authenticated cloud WebUI Bark test succeeded with HTTP 200, and the user confirmed receipt of “Codex Sentinel Lite／Bark 推送通道正常。” on the device. The earlier failed attempt was caused by a stale service environment path and was corrected before the successful test.
- Native delivery records confirm local Pi completion and cloud Grok completion/explicit failure were sent. Both installed systems have zero retired log-maintenance jobs/state keys and no retired configuration section.
- Pi final-failure notification remains intentionally disabled without an extension because native JSONL cannot prove that retry processing has ended. Pi completion and Grok completion/explicit primary-turn failure rules remain covered by tests.

## Repository cleanup

- Only `main` and `v1.0.0` remain as remote branch/tag refs. A fresh clone contains the new root plus normal incremental commits; the pre-rebuild object is absent from the clone and local repository.
- Superseded Releases, tags and Actions runs were removed. Keep the successful implementation CI/Release and subsequent evidence CI; all eight retained cache entries use the new `v1` namespace.
- Source, Git objects, tests and release payload pass privacy checks. The final source/Git/installed-App scan with the external private-value list checked 620 items with zero failures. The private-value list was removed with the task workspace.
- GitHub no longer exposes old refs or Releases, but the pre-rebuild commit object is still addressable by its SHA through GitHub's cache. It requires GitHub Support cache removal; this is reported as a limitation, not claimed as erased.
- Both local App recovery copies, downloads, extracted assets, temporary clone, build outputs and dedicated QA sources were removed after acceptance. Local directory allocation fell by 11,625,181,184 bytes; measured free space increased by 11,334,795,264 bytes. Cloud task artifacts released 61,190,144 allocated bytes. Dependencies and installed runtime/data/configuration remain.
- All 155 pre-existing cloud Grok files retained their hashes; the local Codex index matched its pre-test hash and QA rows/files were absent. Normal delivery/audit records retain their configured expiry. Full checksums and the unsent Support draft are stored outside Git.

## Remaining acceptance limits

- The previous release's destructive deletion acceptance was supplied by the user. This release used only disposable local and cloud QA sessions for batch restore/archive/delete; existing user rows and session indexes were hash-checked or byte-for-byte restored.
- The cloud host has no Pi installation, so native Pi mutation is covered by isolated interfaces and the empty state only.

Future status updates belong here. Do not add release-specific Markdown, backups or deployment secrets.
