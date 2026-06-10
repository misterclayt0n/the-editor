/**
 * Frame-batched rendering: every dirty signal (scroll, notification, resize,
 * highlight arrival) funnels into one requestAnimationFrame callback set, so a
 * burst of events costs exactly one render pass per frame.
 */

const callbacks = new Set<() => void>();
let frameId: number | null = null;

function flush(): void {
  frameId = null;
  const batch = [...callbacks];
  callbacks.clear();
  for (const callback of batch) callback();
}

export function queueRender(callback: () => void): void {
  callbacks.add(callback);
  frameId ??= requestAnimationFrame(flush);
}

export function cancelRender(callback: () => void): void {
  callbacks.delete(callback);
  if (callbacks.size === 0 && frameId !== null) {
    cancelAnimationFrame(frameId);
    frameId = null;
  }
}
