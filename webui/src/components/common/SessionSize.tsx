import type { SessionStorageSize } from "../../types";
import { formatSessionStorage } from "../../lib/domain/sessionStorage";

export function SessionSize({ size }: { size?: SessionStorageSize | null }) {
  const value = formatSessionStorage(size);
  return <span className={`session-size ${value.tone}`} title={value.title} aria-label={value.title} data-size-description={value.title}>{value.label}</span>;
}
