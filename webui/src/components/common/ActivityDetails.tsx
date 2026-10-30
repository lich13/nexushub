import { machineScope } from "../../lib/query/connection";
import { ChevronRight } from "lucide-react";
import { createContext, useContext, useEffect, useState, type ReactNode } from "react";
import { matchesTimelinePosition } from "../../lib/domain/timelineViewModel";

// Only disclosure choices live here, never message contents. The bound prevents
// long-lived desktop sessions from retaining an unbounded set of thread IDs.
type SavedChoice = { open: boolean; seq: number };
const choices = new Map<string, SavedChoice>();
let choiceSeq = 0;

function readSavedChoice(keys: string[]): boolean | undefined {
  let latest: SavedChoice | undefined;
  let minSeq = Infinity;
  let hasOpen = false;
  for (const key of keys) {
    const saved = choices.get(key);
    if (!saved) continue;
    if (saved.open) hasOpen = true;
    if (saved.seq < minSeq) minSeq = saved.seq;
    if (!latest || saved.seq >= latest.seq) latest = saved;
  }
  if (!latest) return undefined;
  // When independently toggled groups merge (different seq stamps), keep open if
  // either group was expanded by the user until the merged group is toggled together.
  if (minSeq < latest.seq && hasOpen) return true;
  return latest.open;
}

export function rememberScopedDisclosure(scope: string, stateKey: string, next: boolean, stateAliases: string[] = []) {
  const id = `${machineScope()}:${scope}:${stateKey}`;
  const aliases = stateAliases.map(key => `${machineScope()}:${scope}:${key}`);
  const seq = ++choiceSeq;
  for (const key of [id, ...aliases]) {
    choices.delete(key);
    choices.set(key, { open: next, seq });
    if (choices.size > 2000) choices.delete(choices.keys().next().value!);
  }
}

export const DisclosureScope = createContext<string | null>(null);
export const DisclosureRevealContext = createContext<string | null>(null);

export function ActivityDetails({ className, stateKey, stateAliases = [], initiallyOpen, summary, children, timelineId, timelineAliases = [], reveal = false }: {
  className: string; stateKey: string; stateAliases?: string[]; initiallyOpen: boolean; summary: ReactNode; children: ReactNode | (() => ReactNode); timelineId?: string; timelineAliases?: string[]; reveal?: boolean;
}) {
  const scope = useContext(DisclosureScope);
  const positionKey = useContext(DisclosureRevealContext);
  const id = scope ? `${machineScope()}:${scope}:${stateKey}` : null;
  const [choice, setChoice] = useState<boolean>();
  const aliases = scope ? stateAliases.map(key => `${machineScope()}:${scope}:${key}`) : [];
  const revealed = reveal || matchesTimelinePosition(positionKey, timelineId, timelineAliases);
  const open = revealed || ((id ? readSavedChoice([id, ...aliases]) : choice) ?? initiallyOpen);
  const remember = (next: boolean) => {
    if (scope) {
      rememberScopedDisclosure(scope, stateKey, next, stateAliases);
    }
    setChoice(next);
  };
  // Mount a search target before its parent locates it, and keep it open after
  // the pending search clears. Other disclosures retain their saved choices.
  useEffect(() => { if (revealed) remember(true); }, [revealed, positionKey, id]);
  return <details className={className} data-timeline-id={timelineId} data-timeline-aliases={timelineAliases.length ? timelineAliases.join(" ") : undefined} open={open} onToggle={event => {
    if (event.target !== event.currentTarget || event.currentTarget.open === open) return;
    remember(event.currentTarget.open);
  }}>
    <summary onClick={event => {
      if ((event.target as Element).closest("button, a, input")) return;
      // Save before a machine/view switch can unmount the element and cancel
      // the browser's asynchronously dispatched toggle event.
      event.preventDefault();
      remember(!open);
    }}>{summary}<ChevronRight className="execution-chevron" size={16} /></summary>
    {typeof children === "function" ? (open ? children() : null) : children}
  </details>;
}
