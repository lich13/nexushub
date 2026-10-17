import { useEffect, useRef, useState } from "react";
import { desktopCatalog, type DesktopCatalogItem } from "../lib/query/desktop";

type Page = { kind: string; items: DesktopCatalogItem[]; cursor: string | null; error: string; loading: boolean };
type State = { key: string; pages: Page[]; busy: boolean; offset: number };
const PAGE = 100;

/** Bounded catalog pages. Request the small installed-plugin directory first. The Rust helper keeps
 * native reads serial, while cached sources and per-source retries can complete
 * independently of a slow application read. */
export function useDesktopReferenceCatalog(sessionId: string | undefined, kind: string, query: string) {
  const key = JSON.stringify([sessionId, kind, query]);
  const [state, setState] = useState<State>({ key: "", pages: [], busy: false, offset: 0 });
  const epoch = useRef(0);
  const fetcher = useRef<((next: boolean, retry?: boolean, retryKind?: string) => Promise<void>) | null>(null);
  useEffect(() => {
    const generation = ++epoch.current;
    fetcher.current = null;
    if (!sessionId || !kind) return;
    const kinds = kind === "mentions" ? ["plugins", "apps"] : [kind];
    let pages: Page[] = kinds.map(value => ({ kind: value, items: [], cursor: null, error: "", loading: false }));
    const publish = () => {
      if (epoch.current === generation) setState({ key, pages: [...pages], busy: pages.some(page => page.loading), offset: 0 });
    };
    const read = async (next: boolean, retry = false, retryKind?: string) => {
      if (epoch.current !== generation) return;
      const retryKinds = new Set(pages.filter(page => page.error).map(page => page.kind));
      const indices = kinds.flatMap((value, index) => {
        if (pages[index].loading || retryKind && retryKind !== value
          || retry && !retryKind && retryKinds.size && !retryKinds.has(value)) return [];
        if (next && !pages[index].cursor) { pages[index] = { ...pages[index], items: [] }; return []; }
        pages[index] = { ...pages[index], error: "", loading: true };
        return [index];
      });
      publish();
      await Promise.all(indices.map(async index => {
        const cursor = next ? pages[index].cursor : null;
        try {
          const result = await desktopCatalog({ sessionId, projectHandle: null, kind: kinds[index], query, cursor, ...(retry ? { forceRefetch: true } : {}) });
          if (epoch.current !== generation) return;
          pages[index] = { kind: kinds[index], items: result.items, cursor: result.nextCursor, error: "", loading: false };
        } catch (reason) {
          if (epoch.current !== generation) return;
          pages[index] = { ...pages[index], loading: false, error: reason instanceof Error ? reason.message : "目录暂不可用，可重试" };
        }
        publish();
      }));
    };
    fetcher.current = read;
    setState({ key, pages, busy: true, offset: 0 });
    const timer = setTimeout(() => { void read(false); }, 250);
    return () => { clearTimeout(timer); ++epoch.current; fetcher.current = null; };
  }, [sessionId, kind, query, key]);
  const current = state.key === key ? state : { key, pages: [], busy: Boolean(kind), offset: 0 };
  const items = current.pages.flatMap(page => page.items);
  const more = () => {
    if (current.busy) return;
    if (current.offset + PAGE < items.length) setState(value => ({ ...value, offset: value.offset + PAGE }));
    else void fetcher.current?.(true);
  };
  return {
    emptyLabel: kind === "apps" ? "没有可用应用" : kind === "plugins" ? "没有可用插件" : kind === "skills" ? "没有可用技能" : "没有可用引用",
    items: items.slice(current.offset, current.offset + PAGE), busy: current.busy,
    loadingKinds: current.pages.filter(page => page.loading).map(page => page.kind),
    errors: current.pages.filter(page => page.error).map(page => ({ kind: page.kind, message: page.error })),
    hasMore: current.offset + PAGE < items.length || current.pages.some(page => page.cursor),
    more, retry: (source?: string) => { void fetcher.current?.(false, true, source); }
  };
}
