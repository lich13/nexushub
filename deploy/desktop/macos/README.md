# NexusHub macOS ARM64

macOS release assets are produced from Tauri bundles. The Tauri build wraps the
main `webui` interface directly, matching the CC Switch native packaging model
without restoring a browser WebUI service.

- `NexusHub-<version>-darwin-arm64.dmg` contains `NexusHub.app` for drag-copy installation.
- `nexushub-darwin-arm64.tar.gz` is an app-only archive containing `NexusHub.app`.
- `nexushub-darwin-arm64.tar.gz.sig` signs the updater archive, and `latest.json`
  advertises it to Tauri as the `darwin-aarch64` platform.
- Each asset has a sibling `.sha256` file in the release output.
- `NexusHub.app` bundles the local `nexushub-webd` helper and syncs it to
  `~/Library/Application Support/NexusHub/bin/nexushub-webd` on launch for Probe
  Bark tests and Hook installation.
- `scripts/package-darwin-arm64.sh` overwrites the tracked helper placeholder
  only during packaging, then restores `src-tauri/resources/nexushub-webd` as a
  non-executable placeholder before exit.

After installing the DMG, validate the App-only surface:

```bash
open -a NexusHub
"$HOME/Library/Application Support/NexusHub/bin/nexushub-webd" --version
tail -n 80 "$HOME/Library/Logs/NexusHub/nexushub.log"
shasum -a 256 -c dist/nexushub-darwin-arm64.tar.gz.sha256
test -s dist/nexushub-darwin-arm64.tar.gz.sig
test -s dist/latest.json
shasum -a 256 -c dist/NexusHub-<version>-darwin-arm64.dmg.sha256
```

This directory is kept as a compatibility note for the retired user-service package.
