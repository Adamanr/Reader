import type { PDFDocumentProxy } from "pdfjs-dist";

/**
 * Страницы одного документа рисуем строго по очереди, как это делает
 * вьюер pdf.js. Параллельная отрисовка нескольких страниц в WebKitGTK
 * иногда оставляла на холсте «дыры» вместо слов.
 */
const chains = new WeakMap<PDFDocumentProxy, Promise<unknown>>();

export function enqueueRender<T>(doc: PDFDocumentProxy, job: () => Promise<T>): Promise<T> {
  const prev = chains.get(doc) ?? Promise.resolve();
  const next = prev.then(job, job);
  chains.set(
    doc,
    next.catch(() => {}),
  );
  return next;
}
