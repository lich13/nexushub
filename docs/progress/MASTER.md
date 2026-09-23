# Current status

- Version: `1.0.0`.
- Scope: three-provider batch management, Grok/Pi Bark extensions, Codex log-maintenance retirement, privacy cleanup and one-time Git history rebuild.
- Current Markdown set: `README.md`, `AGENTS.md`, `DESIGN.md`, `docs/ARCHITECTURE.md`, `docs/cloud-deploy-runbook.md` and this file.

## Implementation

- Parentless release root: `d5e0542f92b1dcc8d35b2fe64f78dd3a88d08ae6`.
- Current incremental migration: `c5611cbd4851aac2ef1c9fbf94ddc4a0ed0c85fd`.
- The migration removes only retired Codex log-maintenance jobs and state keys; provider sessions, configuration, secrets and unrelated jobs remain.
- Batch actions, native Pi JSONL support, provider-specific Bark completion rules, privacy scanning and the systemd session write allowlist are in the release tree.

## Release evidence

- `v1.0.0` has 15 assets, verified sidecar hashes, updater signatures and Darwin/Linux updater mappings.
- The release CI and main CI for the original release root passed. The migration commit has a fresh main CI run in progress; release retagging waits for its complete success.
- The official macOS App and bundled helper report `1.0.0`. The App launches and the Pi batch preview protects an active or uncertain session. The monitor remains port-free after App closure. The package is ad-hoc signed because this host has no Developer ID identities; notarization is not claimed.
- The authenticated cloud WebUI reports `1.0.0`, health is active, the writable Grok session exception is present, and missing Pi storage remains empty without installation. Batch selection, precise deletion preview, ID copy, provider Bark settings and retired-route 404s were verified. No live user session was deleted during this acceptance.
- A real cloud Bark test was attempted and failed; the server recorded only a redacted failure marker. Existing delivery history and the provider setting paths are intact, but device receipt and a fresh successful push remain unverified.
- Pi final-failure notification remains intentionally disabled without an extension because native JSONL cannot prove that retry processing has ended. Pi completion and Grok completion/explicit primary-turn failure rules remain covered by tests.

## Repository cleanup

- `main` and `v1.0.0` point to the release line; only the new root plus normal incremental commits remain locally and remotely.
- Legacy Releases, tags, Actions runs and old cache entries were removed. Two current Actions runs and the new release cache namespace remain.
- The source, Git objects, tests and release payload pass `scripts/privacy-check.py --git-objects`; the final scan reports no private paths, public deployment addresses, private email, credentials or real session content.
- GitHub no longer exposes old refs or Releases, but the pre-rebuild commit object is still addressable by its SHA through GitHub's cache. It requires GitHub Support cache removal; this is reported as a limitation, not claimed as erased.

## Remaining acceptance limits

- The previous release's destructive deletion acceptance was supplied by the user. This release verified batch previews and isolated mixed-result tests; live deletion was intentionally limited to disposable or preview-only data.
- Cloud Bark delivery needs a fresh successful device-side confirmation. The failed test is preserved as an explicit gap and was not retried automatically.
- The cloud host has no Pi installation, so native Pi mutation is covered by isolated interfaces and the empty state only.

Future status updates belong here. Do not add release-specific Markdown, backups or deployment secrets.
