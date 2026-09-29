import { machineScope } from "../../lib/query/connection";
import { ChevronRight } from "lucide-react";
import { createContext, useContext, useState, type ReactNode } from "react";

// Only disclosure choices live here, never message contents. The bound prevents
// long-lived desktop sessions from retaining an unbounded set of thread IDs.
const choices = new Map<string, boolean>();
export const DisclosureScope = createContext<string | null>(null);

export function ActivityDetails({ className, stateKey, stateAliases = [], initiallyOpen, summary, children }: {
  className: string; stateKey: string; stateAliases?: string[]; initiallyOpen: boolean; summary: ReactNode; children: ReactNode | (() => ReactNode);
}) {
  const scope = useContext(DisclosureScope);
  const id = scope ? `${machineScope()}:${scope}:${stateKey}` : null;
  const [choice, setChoice] = useState<boolean>();
  const aliases = scope ? stateAliases.map(key => `${machineScope()}:${scope}:${key}`) : [];
  const open = (id ? choices.get(id) ?? aliases.map(key => choices.get(key)).find(value => value !== undefined) : choice) ?? initiallyOpen;
  return <details className={className} open={open} onToggle={event => {
    if (event.target !== event.currentTarget || event.currentTarget.open === open) return;
    const next = event.currentTarget.open;
    if (id) {
      for (const key of [id, ...aliases]) {
        choices.delete(key);
        choices.set(key, next);
        if (choices.size > 2000) choices.delete(choices.keys().next().value!);
      }
    }
    setChoice(next);
  }}>
    <summary>{summary}<ChevronRight className="execution-chevron" size={16} /></summary>
    {typeof children === "function" ? (open ? children() : null) : children}
  </details>;
}
