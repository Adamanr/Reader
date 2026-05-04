/** Пути проекта Typst рядом с исходной книгой: `dir/Stem.reader.typ/book.typ`. */

export function typstProjectDirFromSource(sourceRelative: string): string {
  const n = sourceRelative.replace(/\\/g, "/");
  const slash = n.lastIndexOf("/");
  const dir = slash >= 0 ? n.slice(0, slash + 1) : "";
  const file = slash >= 0 ? n.slice(slash + 1) : n;
  const dot = file.lastIndexOf(".");
  const stem = dot > 0 ? file.slice(0, dot) : file;
  return `${dir}${stem}.reader.typ`;
}

export function typstMainFileRelative(sourceRelative: string): string {
  return `${typstProjectDirFromSource(sourceRelative)}/book.typ`;
}

export function typstThemeFileRelative(sourceRelative: string): string {
  return `${typstProjectDirFromSource(sourceRelative)}/theme.typ`;
}
