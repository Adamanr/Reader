/** Файл рядом с конфигом приложения: pdf-translations/<sha256(path)>.json */

export type PdfBookTranslationStatus = "running" | "complete" | "interrupted";

export interface PdfBookTranslationFile {
  version: 1;
  status: PdfBookTranslationStatus;
  baseUrl: string;
  model: string;
  sourceLang: string;
  targetLang: string;
  temperature: number;
  maxTokens: number | null;
  totalPages: number;
  /** Ключ — номер страницы (1-based), массив строк в порядке pdf.js TextLayer */
  pages: Record<string, string[]>;
  /** Последняя обрабатываемая страница при сохранении */
  lastProcessedPage?: number;
  updatedAt: string;
}

export function parsePdfBookTranslationFile(raw: string): PdfBookTranslationFile | null {
  try {
    const o = JSON.parse(raw) as Partial<PdfBookTranslationFile>;
    if (o.version !== 1 || typeof o.pages !== "object" || o.pages == null) return null;
    return o as PdfBookTranslationFile;
  } catch {
    return null;
  }
}
