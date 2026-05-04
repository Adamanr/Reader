export type Importance = "low" | "normal" | "high" | "essential";

export interface Shelf {
  id: string;
  name: string;
  order: number;
}

export interface BookComment {
  id: string;
  body: string;
  page?: number;
  chapterLabel?: string;
  excerpt?: string;
  createdAt: string;
}

export type QuoteAccent = "sand" | "sage" | "dustyRose" | "ink";
export type QuoteLayout = "wide" | "narrow" | "center";
export type QuoteBgFit = "cover" | "contain";

export interface SavedQuote {
  id: string;
  text: string;
  createdAt: string;
  accent: QuoteAccent | string;
  layout: QuoteLayout | string;
  includePage: boolean;
  includeChapter: boolean;
  /** Подпись на карточке при сохранении (может отличаться от метаданных книги). */
  bookTitle?: string;
  bookAuthor?: string;
  /** Фон-картинка (data URL, ужатая на клиенте). */
  bgImageDataUrl?: string | null;
  /** 0…1, непрозрачность слоя картинки */
  bgImageOpacity?: number | null;
  /** Цвет тинта поверх картинки (#rrggbb или rgba) */
  overlayColor?: string | null;
  /** 0…1, непрозрачность тинта */
  overlayOpacity?: number | null;
  /** Масштаб фона, ~100 = без доп. zoom */
  bgScale?: number | null;
  bgFit?: QuoteBgFit | string | null;
}

export interface QuoteDraftOptions {
  accent: QuoteAccent;
  layout: QuoteLayout;
  includePage: boolean;
  includeChapter: boolean;
  bookTitle: string;
  bookAuthor: string;
  bgImageDataUrl?: string | null;
  bgImageOpacity: number;
  overlayColor: string;
  overlayOpacity: number;
  bgScale: number;
  bgFit: QuoteBgFit;
}

export interface BookMeta {
  path: string;
  /** Основная полка (для совместимости и сортировки). */
  shelfId: string;
  /** Книга может быть на нескольких полках; если пусто — только `shelfId`. */
  shelfIds?: string[];
  importance: Importance | string;
  review: string;
  title?: string | null;
  author?: string | null;
  comments?: BookComment[];
  quotes?: SavedQuote[];
  /** Последняя страница PDF (1-based). */
  lastReadPdfPage?: number | null;
  lastReadPdfTotal?: number | null;
  /** EPUB: spine href; FB2: `#anchor`. */
  lastReadLocation?: string | null;
  lastReadLocationLabel?: string | null;
  /** Сохранён PDF translate_* рядом с исходником (полный перевод). */
  translationExported?: boolean | null;
  /** Миниатюра обложки (data URL), для списка книг; подставляется при первом просмотре. */
  coverThumbDataUrl?: string | null;
  /** ISO-время последнего открытия в читалке (для сортировки «недавние»). */
  lastOpenedAt?: string | null;
  /**
   * Путь к файлу стиля `.typ` в папке библиотеки (например `.reader-typst-themes/minimal.typ`).
   * Если не задан — при экспорте в Typst подставляется общий стиль из настроек; сам проект всё равно содержит локальный `theme.typ`.
   */
  typstStyleRelativePath?: string | null;
}

export interface LibraryMetadata {
  shelves: Shelf[];
  books: Record<string, BookMeta>;
}

export interface LibrarySnapshot {
  libraryRoot: string | null;
  bookPaths: string[];
  metadata: LibraryMetadata;
  /** Общий стиль Typst по умолчанию (путь относительно корня библиотеки). */
  defaultTypstStyleRelativePath?: string | null;
}

export const IMPORTANCE_OPTIONS: { value: Importance; label: string }[] = [
  { value: "low", label: "Низкая" },
  { value: "normal", label: "Обычная" },
  { value: "high", label: "Высокая" },
  { value: "essential", label: "Обязательно" },
];

export interface PdfOutlineItem {
  title: string;
  page: number | null;
}

export interface PdfReadyInfo {
  outline: PdfOutlineItem[];
  numPages: number;
}

export interface EpubReaderApi {
  toc: { label: string; href: string }[];
  spine: { label: string; href: string }[];
  goTo: (href: string) => Promise<void>;
  prev: () => Promise<void>;
  next: () => Promise<void>;
}
