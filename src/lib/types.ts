export type Importance = "low" | "normal" | "high" | "essential";

export interface Shelf {
  id: string;
  name: string;
  order: number;
}

export type ReadingStatus = "want" | "reading" | "done" | "dropped";

export const READING_STATUS_OPTIONS: { value: ReadingStatus; label: string }[] = [
  { value: "want", label: "Хочу прочитать" },
  { value: "reading", label: "Читаю" },
  { value: "done", label: "Прочитано" },
  { value: "dropped", label: "Отложено" },
];

export type HighlightColor = "yellow" | "green" | "blue" | "pink" | "violet";

export interface Highlight {
  id: string;
  text: string;
  color: HighlightColor;
  /** Заметка на полях (необязательно). */
  note?: string;
  /** PDF: страница (1-based). */
  page?: number;
  /** EPUB: CFI-диапазон выделения. */
  cfi?: string;
  /** FB2: индекс блока (абзаца) и смещение текста внутри него. */
  block?: number;
  /** Смещение начала в тексте блока/страницы (для поиска фрагмента). */
  offset?: number;
  chapterLabel?: string;
  createdAt: string;
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
  /** Текст после правки в редакторе карточки. */
  text: string;
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
  /** Скрыта из основной библиотеки, но остаётся на диске. */
  hidden?: boolean;
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
  /** Статус чтения; если не задан — выводится из прогресса. */
  status?: ReadingStatus | null;
  /** Общий прогресс 0…1 для всех форматов. */
  progress?: number | null;
  /** EPUB: href текущей главы (для оглавления), когда lastReadLocation — CFI. */
  lastReadHref?: string | null;
  highlights?: Highlight[];
  /** Время добавления в библиотеку (мс, ставит бэкенд). */
  addedAtMs?: number | null;
  finishedAt?: string | null;
  /** Метаданные файла заполнены автоматически (не перезаписывать ручные). */
  autoMetaDone?: boolean | null;
  /** Оценка оставшегося времени чтения, минуты (обновляется при чтении). */
  minutesLeft?: number | null;
  /** Фоновый звук, привязанный к книге. */
  ambientSound?: string | null;
}

export interface LibraryMetadata {
  shelves: Shelf[];
  books: Record<string, BookMeta>;
}

export interface LibrarySnapshot {
  libraryRoot: string | null;
  bookPaths: string[];
  hiddenBookPaths: string[];
  metadata: LibraryMetadata;
  /** Общий стиль Typst по умолчанию (путь относительно корня библиотеки). */
  defaultTypstStyleRelativePath?: string | null;
  /** Данные лежат в `<библиотека>/.reader` и синхронизируются вместе с книгами. */
  syncInLibrary?: boolean;
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
  /** Уровень заголовка в дереве PDF, начиная с 0. */
  level: number;
}

export interface ReaderOutlineItem {
  id: string;
  label: string;
  /** Исходный уровень заголовка; компонент сам нормализует первый уровень. */
  level: number;
  meta?: string;
  disabled?: boolean;
}

export interface PdfReadyInfo {
  outline: PdfOutlineItem[];
  numPages: number;
}

/** Позиция чтения, которую сообщает любой просмотрщик. */
export interface ReadingPosition {
  /** 0…1, если известно */
  progress: number | null;
  chapterLabel: string;
  /** Символов до конца главы / книги (EPUB, FB2) */
  charsLeftChapter: number | null;
  charsLeftBook: number | null;
  /** PDF: страниц до конца главы / книги */
  pagesLeftChapter?: number | null;
  pagesLeftBook?: number | null;
  /** Подпись позиции: «стр. 12 / 300» */
  pageLabel?: string;
}

export interface SearchHit {
  id: string;
  label: string;
  before: string;
  match: string;
  after: string;
  /** Адрес внутри книги: CFI, `p:<страница>:<смещение>` или `s:<секция>:<смещение>` */
  loc: string;
}

/** Выделение текста, о котором просмотрщик сообщает странице чтения. */
export interface ReaderSelection {
  text: string;
  /** Прямоугольник выделения в координатах окна */
  rect: { left: number; top: number; width: number; height: number };
  /** Привязка для постоянного выделения */
  anchor: Pick<Highlight, "page" | "cfi" | "block" | "offset" | "chapterLabel">;
  /** Текст абзаца вокруг выделения (для словаря и пояснений) */
  context?: string;
  clear: () => void;
}

/** Абзац для чтения вслух, RSVP и режима фокуса. */
export interface ReadingUnit {
  text: string;
  /** Подсветить абзац как текущий */
  mark?: () => void;
  unmark?: () => void;
  /** Показать абзац на экране (перелистнуть при необходимости) */
  reveal?: () => Promise<void>;
}

/** Кусок текста книги до текущей позиции (для пересказа и глоссария без спойлеров). */
export interface TextChunk {
  id: string;
  label: string;
  text: string;
  /** Глава прочитана не до конца */
  partial: boolean;
}

export interface EpubReaderApi {
  toc: { label: string; href: string; level: number }[];
  spine: { label: string; href: string }[];
  goTo: (href: string) => Promise<void>;
  prev: () => Promise<void>;
  next: () => Promise<void>;
  /** Перейти к доле книги 0…1 */
  seek?: (fraction: number) => Promise<void>;
  search?: (query: string, signal?: AbortSignal) => Promise<SearchHit[]>;
  goToHit?: (hit: SearchHit) => Promise<void>;
  /** Абзацы от текущего места до конца главы */
  unitsFromHere?: () => Promise<ReadingUnit[]>;
  /** Следующая глава после `unitsFromHere` (для непрерывного чтения вслух) */
  advanceChapter?: () => Promise<boolean>;
  chunksBefore?: () => Promise<TextChunk[]>;
}

export type ReaderApi = EpubReaderApi;
