import { useEffect, useRef, useState } from "react";
import { desktopCatalog, type DesktopCatalogItem } from "../lib/query/desktop";

type Page = { kind: string; items: DesktopCatalogItem[]; cursor: string | null; error: string };
type State = { key: string; pages: Page[]; busy: boolean; offset: number };
const PAGE = 100;

/** Bounded catalog pages. Mention sources are fetched sequentially; a failed
 * app directory must not discard a successful plugin directory (or vice versa). */
export function useDesktopReferenceCatalog(sessionId: string | undefined, kind: string, query: string) {
  const key = JSON.stringify([sessionId, kind, query]);
  const [state, setState] = useState<State>({ key: "", pages: [], busy: false, offset: 0 });
  const [attempt, setAttempt] = useState(0);
  const epoch = useRef(0);
  const fetcher = useRef<((next: boolean) => Promise<void>) | null>(null);
  useEffect(() => {
    const generation = ++epoch.current;
    fetcher.current = null;
    if (!sessionId || !kind) return;
    const kinds = kind === "mentions" ? ["apps", "plugins"] : [kind];
    let pages: Page[] = kinds.map(value => ({ kind: value, items: [], cursor: null, error: "" }));
    let busy = false;
    const publish = () => {
      if (epoch.current === generation) setState({ key, pages: [...pages], busy, offset: 0 });
    };
    const read = async (next: boolean) => {
      if (busy || epoch.current !== generation) return;
      busy = true; publish();
      for (let index = 0; index < kinds.length; index++) {
        if (epoch.current !== generation) return;
        const cursor = next ? pages[index].cursor : null;
        if (next && !cursor) { pages[index] = { ...pages[index], items: [] }; continue; }
        try {
          const result = await desktopCatalog({ sessionId, projectHandle: null, kind: kinds[index], query, cursor });
          if (epoch.current !== generation) return;
          pages[index] = { kind: kinds[index], items: result.items, cursor: result.nextCursor, error: "" };
        } catch (reason) {
          if (epoch.current !== generation) return;
          pages[index] = { ...pages[index], error: reason instanceof Error ? reason.message : "目录暂不可用，可重试" };
        }
        publish();
      }
      busy = false; publish();
    };
    fetcher.current = read;
    setState({ key, pages, busy: true, offset: 0 });
    const timer = setTimeout(() => { void read(false); }, 250);
    return () => { clearTimeout(timer); ++epoch.current; fetcher.current = null; };
  }, [sessionId, kind, query, key, attempt]);
  const current = state.key === key ? state : { key, pages: [], busy: Boolean(kind), offset: 0 };
  const items = current.pages.flatMap(page => page.items);
  const more = () => {
    if (current.busy) return;
    if (current.offset + PAGE < items.length) setState(value => ({ ...value, offset: value.offset + PAGE }));
    else void fetcher.current?.(true);
  };
  return {
    items: items.slice(current.offset, current.offset + PAGE), busy: current.busy,
    errors: current.pages.filter(page => page.error).map(page => ({ kind: page.kind, message: page.error })),
    hasMore: current.offset + PAGE < items.length || current.pages.some(page => page.cursor),
    more, retry: () => setAttempt(value => value + 1)
  };
}
