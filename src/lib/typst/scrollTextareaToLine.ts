/** Прокрутка и курсор на строку `line1Based` в textarea. */
export function scrollTextareaToLine(el: HTMLTextAreaElement, line1Based: number): void {
  const lines = el.value.split("\n");
  const maxLine = Math.max(1, lines.length);
  const idx = Math.min(Math.max(line1Based, 1), maxLine);
  let pos = 0;
  for (let i = 0; i < idx - 1; i++) pos += lines[i]!.length + 1;
  el.focus();
  el.setSelectionRange(pos, pos);
  const lhRaw = getComputedStyle(el).lineHeight;
  const lh =
    lhRaw === "normal"
      ? parseFloat(getComputedStyle(el).fontSize || "14") * 1.45
      : parseFloat(lhRaw) || 20;
  el.scrollTop = Math.max(0, (idx - 1) * lh - el.clientHeight * 0.35);
}
