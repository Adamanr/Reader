import type { QuoteAccent, QuoteLayout } from "$lib/types";

export const ACCENT_PREVIEW_BG: Record<QuoteAccent, string> = {
  sand: "linear-gradient(145deg, #faf5ee 0%, #efe6dc 55%, #e8dfd4 100%)",
  sage: "linear-gradient(145deg, #f0f4f1 0%, #e2eae3 55%, #d8e3da 100%)",
  dustyRose: "linear-gradient(145deg, #faf4f6 0%, #f0e4ea 55%, #e8d9e1 100%)",
  ink: "linear-gradient(145deg, #3a3632 0%, #2d2926 55%, #24201e 100%)",
};

export const ACCENT_QUOTE_COLOR: Record<QuoteAccent, string> = {
  sand: "#2f2a26",
  sage: "#253028",
  dustyRose: "#3d2c34",
  ink: "#f7f2ea",
};

/** Canvas gradient [top, bottom] for PNG export */
export const ACCENT_CANVAS_GRADIENT: Record<QuoteAccent, [string, string]> = {
  sand: ["#faf5ee", "#e8dfd4"],
  sage: ["#f0f4f1", "#d8e3da"],
  dustyRose: ["#faf4f6", "#e8d9e1"],
  ink: ["#3a3632", "#24201e"],
};

/** Sidebar stripe / chips */
export const ACCENT_CHIP_BG: Record<QuoteAccent, string> = {
  sand: "linear-gradient(180deg, #e8ddd2, #dccfbff2)",
  sage: "linear-gradient(180deg, #d5e0d6, #c5d2c8f2)",
  dustyRose: "linear-gradient(180deg, #ead6dd, #ddc4cef2)",
  ink: "linear-gradient(180deg, #4a4540, #35302cf2)",
};

export function previewMaxWidth(layout: QuoteLayout): string {
  if (layout === "narrow") return "min(340px, 100%)";
  if (layout === "center") return "min(480px, 100%)";
  return "100%";
}

export function previewMargin(layout: QuoteLayout): string {
  return layout === "center" ? "0 auto" : "0";
}

export function exportContentWidth(layout: QuoteLayout): number {
  if (layout === "narrow") return 380;
  if (layout === "center") return 520;
  return 680;
}
