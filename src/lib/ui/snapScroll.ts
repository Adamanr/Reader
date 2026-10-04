/**
 * Плавная прокрутка колёсиком в WebKitGTK (с ускорением на видеокарте) может
 * остановиться на дробной позиции: слой с текстом остаётся размытым, пока
 * прокрутку не сдвинуть ещё раз. После остановки переустанавливаем позицию
 * с главного потока — компоновщик встаёт на целый пиксель, текст снова чёткий.
 */
export function snapScroll(el: HTMLElement): () => void {
  let timer: ReturnType<typeof setTimeout> | null = null;
  let ignoreUntil = 0;
  let lastSnapped = "";

  const snap = () => {
    timer = null;
    const top = el.scrollTop;
    const left = el.scrollLeft;
    const key = `${top}|${left}`;
    if (key === lastSnapped) return;
    lastSnapped = key;
    const maxTop = el.scrollHeight - el.clientHeight;
    if (maxTop <= 0) return;
    const nudge = top >= maxTop ? top - 1 : top + 1;
    ignoreUntil = performance.now() + 120;
    el.scrollTo({ top: nudge, left, behavior: "instant" });
    el.scrollTo({ top, left, behavior: "instant" });
  };

  const onScroll = () => {
    if (performance.now() < ignoreUntil) return;
    if (timer) clearTimeout(timer);
    timer = setTimeout(snap, 180);
  };

  el.addEventListener("scroll", onScroll, { passive: true });
  return () => {
    el.removeEventListener("scroll", onScroll);
    if (timer) clearTimeout(timer);
  };
}
