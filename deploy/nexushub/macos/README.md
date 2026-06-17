# NexusHub macOS ARM64

macOS release assets are produced from Tauri bundles. The Tauri build wraps the
main `webui` interface directly, matching the CC Switch native packaging model
without restoring a browser WebUI service.

- `NexusHub-<version>-darwin-arm64.dmg` contains `NexusHub.app` for drag-copy installation.
- `nexushub-darwin-arm64.tar.gz` is an app-only archive containing `NexusHub.app`.
- Each asset has a sibling `.sha256` file in the release output.

This directory is kept as a compatibility note for the retired user-service package.
