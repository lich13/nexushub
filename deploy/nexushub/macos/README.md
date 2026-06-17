# NexusHub macOS ARM64

Install from the extracted tarball or mounted DMG:

```bash
./install.sh
```

When using the DMG, you can also double-click `Install.command`. After the
service is running, `NexusHub.app` opens the local WebUI at
`http://127.0.0.1:15742/nexushub/`.

The installer is user-level and does not need sudo. It installs NexusHub into `~/Library/Application Support/NexusHub`, writes logs under `~/Library/Logs/NexusHub`, and registers `~/Library/LaunchAgents/com.nexushub.nexushub.plist`.

After launch, open `http://127.0.0.1:15742/nexushub/`.

Uninstall the LaunchAgent and installed binaries while keeping data:

```bash
./uninstall.sh
```

From the DMG, `Uninstall.command` runs the same user-level uninstall entry.

Remove data and logs as well:

```bash
REMOVE_DATA=1 ./uninstall.sh
```
