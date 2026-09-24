import type { HighlightColor } from "$lib/types";

export const HIGHLIGHT_COLORS: { id: HighlightColor; label: string; fill: string }[] = [
  { id: "yellow", label: "Жёлтый", fill: "#f5d565" },
  { id: "green", label: "Зелёный", fill: "#94d49a" },
  { id: "blue", label: "Голубой", fill: "#8ec5ee" },
  { id: "pink", label: "Розовый", fill: "#f2a3c0" },
  { id: "violet", label: "Сиреневый", fill: "#c3a8f0" },
];

export function highlightFill(color: string): string {
  return HIGHLIGHT_COLORS.find((c) => c.id === color)?.fill ?? HIGHLIGHT_COLORS[0]!.fill;
}
