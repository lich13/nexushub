/** Native events already provide progress. Coalesce them without cancelling a
 * slow detail request, then deliver one trailing refresh for events it missed. */
export function desktopRefresh(refresh: () => Promise<unknown>, current: () => boolean) {
  let timer: ReturnType<typeof setTimeout> | undefined;
  let disposed = false;
  let running = false;
  let dirty = false;
  const valid = () => !disposed && current();
  const schedule = () => {
    if (!valid() || running || timer !== undefined || !dirty) return;
    timer = setTimeout(() => { void run(); }, 350);
  };
  const run = async () => {
    timer = undefined;
    if (!valid()) return;
    dirty = false; running = true;
    try { await refresh(); }
    catch { /* The existing detail query owns read errors and retry policy. */ }
    finally { running = false; schedule(); }
  };
  return {
    request: () => { if (valid()) { dirty = true; schedule(); } },
    dispose: () => { disposed = true; dirty = false; clearTimeout(timer); }
  };
}
