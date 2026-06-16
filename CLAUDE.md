# Claude Code Notes

Follow `AGENTS.md` first. These notes are specific to Claude Code or Claude-oriented handoffs.

- Use `docs/progress/MASTER.md` for project status, and verify GitHub/release state from `https://github.com/lich13/nexushub`.
- Treat `https://661313.xyz/nexushub/` as the deployed NexusHub path; the legacy `/codex-cloud-panel/` path has been intentionally retired and should stay `404`.
- Preserve Codex provider behavior while adding Claude Code framework pieces. Codex remains the complete provider surface for this release.
- Verify Codex local-state reads through the resolved Codex home first: state DB, `session_index.jsonl`, rollout files, and `logs_2.sqlite`. Control paths use fixed local jobs and should be visible in job history when invoked.
- Claude Code provider work is currently read-only. Safe reads include `~/.claude/projects`, session JSONL metadata, and redacted `~/.claude/settings.json`; writes to Claude settings or tool permissions require an explicit future task.
- Do not copy AGPL code, schemas, assets, or plugin ABI from external Claude UI projects. Recreate architecture and interaction patterns independently.
- Probe is built into NexusHub for cloud replacement of the old `codex-sentinel-server` runtime. Preserve the same safety boundary as AGENTS.md: visible/observable maintenance only, no hidden desktop control, no auto-reply, no arbitrary shell.
