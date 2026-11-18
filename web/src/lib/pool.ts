/**
 * Jalankan `worker` untuk tiap item, maksimal `limit` yang berjalan bersamaan.
 *
 * `next++` aman tanpa lock: JavaScript single-threaded, dan worker cuma
 * gantian di titik `await`.
 */
export async function runPool<T>(
  items: readonly T[],
  limit: number,
  worker: (item: T) => Promise<void>,
): Promise<void> {
  let next = 0
  const runner = async () => {
    while (next < items.length) {
      await worker(items[next++])
    }
  }
  await Promise.all(Array.from({ length: Math.min(limit, items.length) }, runner))
}
