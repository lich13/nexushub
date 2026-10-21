export function claudeResumeCommand(cwd: string, id: string): string {
  const quote = (value: string) => `'${value.replaceAll("'", "'\\''")}'`;
  return `${cwd ? `cd -- ${quote(cwd)} && ` : ""}claude --resume ${quote(id)}`;
}
