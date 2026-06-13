# Claude Code Notes

Follow `AGENTS.md` first. These notes are specific to Claude Code or Claude-oriented handoffs.

- Use LOCAL_ONLY progress tracking in `docs/progress/MASTER.md` unless a real GitHub remote and valid auth are configured.
- Treat `lich13/nexushub` as the intended upstream name, not proof that the local checkout is linked to a remote.
- Preserve Codex provider behavior while adding Claude Code framework pieces. Codex remains the complete provider surface for this release.
- Verify Codex control through the app-server bridge first. `codex exec --json` is a fallback path only and should be visible in job history when used.
- Claude Code provider work is currently read-only. Safe reads include `~/.claude/projects`, session JSONL metadata, and redacted `~/.claude/settings.json`; writes to Claude settings or tool permissions require an explicit future task.
- Do not copy AGPL code, schemas, assets, or plugin ABI from external Claude UI projects. Recreate architecture and interaction patterns independently.

