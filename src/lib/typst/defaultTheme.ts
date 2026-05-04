/** Минимальная тема по умолчанию, если в библиотеке нет выбранного файла. */
export const BUILTIN_MINIMAL_THEME = `// Тема оформления книги (редактируйте и сохраняйте)
#let apply-book-theme(body) = {
  set page(
    width: 148mm,
    height: 210mm,
    margin: (x: 1.8cm, y: 2.2cm),
  )
  set text(font: "Libertinus Serif", size: 11pt, lang: "ru")
  set par(justify: true, leading: 0.65em)
  set heading(numbering: "1.1")
  show heading.where(level: 1): it => {
    v(0.6em)
    it
    v(0.35em)
  }
  body
}
`;
