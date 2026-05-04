/** Экранирование текста в разметке Typst (markup). */
export function escapeTypstMarkup(s: string): string {
  return s.replace(/[\\#$*_`[\]<>]/g, "\\$&");
}
