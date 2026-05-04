/** Очередь генерации обложек — не запускает десятки pdf.js подряд на главном потоке. */
let chain: Promise<unknown> = Promise.resolve();

export function enqueueCoverGeneration<T>(fn: () => Promise<T>): Promise<T> {
  const next = chain.then(fn, () => fn());
  chain = next.then(
    () => {},
    () => {},
  );
  return next;
}
