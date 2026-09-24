import type { FontId } from "$lib/reading/fonts";

/** Тема страницы — отдельно от темы интерфейса. */
export type PageThemeId = "app" | "paper" | "sepia" | "graphite" | "oled" | "cover";

export type FocusMode = "off" | "paragraph" | "line";

export interface ReadingSettings {
  font: FontId;
  /** Кегль, px */
  fontSize: number;
  lineHeight: number;
  /** Длина строки, символов */
  measure: number;
  /** Поля по бокам, px */
  margin: number;
  align: "left" | "justify";
  hyphens: boolean;
  /** Интервал между абзацами, em */
  paraSpacing: number;
  indent: boolean;
  /** Разрядка для «лёгкого чтения», em */
  letterSpacing: number;
  wordSpacing: number;
  pageTheme: PageThemeId;
  epubFlow: "paginated" | "scrolled";
  pdfMode: "continuous" | "single" | "spread";
  pdfFit: "width" | "page" | "custom";
  /** Масштаб PDF для режима custom */
  pdfZoom: number;
  /** Инверсия страниц PDF в тёмных темах */
  pdfInvert: boolean;
  /** Обрезать белые поля PDF */
  pdfCrop: boolean;
  tapZones: boolean;
  focus: FocusMode;
  /** Интерфейс подстраивает акцент под обложку книги */
  livingCover: boolean;
  /** Скорость RSVP, слов в минуту */
  rsvpWpm: number;
}

export const DEFAULT_READING_SETTINGS: ReadingSettings = {
  font: "literata",
  fontSize: 19,
  lineHeight: 1.62,
  measure: 66,
  margin: 32,
  align: "left",
  hyphens: true,
  paraSpacing: 0.35,
  indent: true,
  letterSpacing: 0,
  wordSpacing: 0,
  pageTheme: "app",
  epubFlow: "paginated",
  pdfMode: "continuous",
  pdfFit: "width",
  pdfZoom: 1,
  pdfInvert: false,
  pdfCrop: false,
  tapZones: true,
  focus: "off",
  livingCover: true,
  rsvpWpm: 320,
};

const STORAGE_KEY = "reader.readingSettings.v1";

function load(): ReadingSettings {
  try {
    const raw = typeof localStorage !== "undefined" ? localStorage.getItem(STORAGE_KEY) : null;
    if (raw) return { ...DEFAULT_READING_SETTINGS, ...(JSON.parse(raw) as Partial<ReadingSettings>) };
  } catch {
    /* ignore */
  }
  return { ...DEFAULT_READING_SETTINGS };
}

/** Настройки чтения этого устройства (как у бумажной книги — у каждого экрана свои). */
export const reading = $state<{ s: ReadingSettings }>({ s: load() });

export function updateReading(patch: Partial<ReadingSettings>) {
  reading.s = { ...reading.s, ...patch };
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(reading.s));
  } catch {
    /* ignore */
  }
}

export function resetTypography() {
  const d = DEFAULT_READING_SETTINGS;
  updateReading({
    font: d.font,
    fontSize: d.fontSize,
    lineHeight: d.lineHeight,
    measure: d.measure,
    margin: d.margin,
    align: d.align,
    hyphens: d.hyphens,
    paraSpacing: d.paraSpacing,
    indent: d.indent,
    letterSpacing: d.letterSpacing,
    wordSpacing: d.wordSpacing,
  });
}

/** Пресеты — быстрые наборы для разных ситуаций. */
export const TYPO_PRESETS: { id: string; label: string; patch: Partial<ReadingSettings> }[] = [
  {
    id: "book",
    label: "Книжный",
    patch: {
      font: "literata",
      fontSize: 19,
      lineHeight: 1.62,
      measure: 66,
      align: "justify",
      hyphens: true,
      indent: true,
      paraSpacing: 0.15,
      letterSpacing: 0,
      wordSpacing: 0,
    },
  },
  {
    id: "airy",
    label: "Воздушный",
    patch: {
      font: "lora",
      fontSize: 20,
      lineHeight: 1.85,
      measure: 58,
      align: "left",
      hyphens: false,
      indent: false,
      paraSpacing: 0.9,
      letterSpacing: 0,
      wordSpacing: 0,
    },
  },
  {
    id: "easy",
    label: "Лёгкое чтение",
    patch: {
      font: "ptsans",
      fontSize: 21,
      lineHeight: 1.9,
      measure: 54,
      align: "left",
      hyphens: false,
      indent: false,
      paraSpacing: 1,
      letterSpacing: 0.04,
      wordSpacing: 0.16,
    },
  },
  {
    id: "dense",
    label: "Плотный",
    patch: {
      font: "ptserif",
      fontSize: 17,
      lineHeight: 1.45,
      measure: 78,
      align: "justify",
      hyphens: true,
      indent: true,
      paraSpacing: 0,
      letterSpacing: 0,
      wordSpacing: 0,
    },
  },
];
