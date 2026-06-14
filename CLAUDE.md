# Claude Code Notes

Follow `AGENTS.md` first. These notes are specific to Claude Code or Claude-oriented handoffs.

- Use `docs/progress/MASTER.md` for project status, and verify GitHub/release state from `https://github.com/lich13/nexushub`.
- Treat `https://661313.xyz/nexushub/` as the deployed NexusHub path; do not replace the legacy `/codex-cloud-panel/` path without an explicit migration task.
- Preserve Codex provider behavior while adding Claude Code framework pieces. Codex remains the complete provider surface for this release.
- Verify Codex control through the app-server bridge first. `codex exec --json` is a fallback path only and should be visible in job history when used.
- Claude Code provider work is currently read-only. Safe reads include `~/.claude/projects`, session JSONL metadata, and redacted `~/.claude/settings.json`; writes to Claude settings or tool permissions require an explicit future task.
- Do not copy AGPL code, schemas, assets, or plugin ABI from external Claude UI projects. Recreate architecture and interaction patterns independently.
- Probe is built into NexusHub for cloud replacement of the old `codex-sentinel-server` runtime. Preserve the same safety boundary as AGENTS.md: visible/observable maintenance only, no hidden desktop control, no auto-reply, no arbitrary shell.
