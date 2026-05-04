import { getDocument, GlobalWorkerOptions } from "pdfjs-dist";
import pdfWorker from "pdfjs-dist/build/pdf.worker.min.mjs?url";
import { extractReadablePageText } from "$lib/pdf/readablePageText";
import { escapeTypstMarkup } from "./escapeTypst";

GlobalWorkerOptions.workerSrc = pdfWorker;

export async function pdfBytesToTypstBody(bytes: Uint8Array): Promise<string> {
  const pdf = await getDocument({ data: bytes }).promise;
  const chunks: string[] = [];
  for (let p = 1; p <= pdf.numPages; p++) {
    const page = await pdf.getPage(p);
    const text = await extractReadablePageText(page, 1);
    const block = text.trim();
    if (block) {
      chunks.push(escapeTypstMarkup(block).replace(/\n\n+/g, "\n\n"));
    }
    if (p < pdf.numPages) chunks.push("#pagebreak()");
  }
  return chunks.join("\n\n");
}
