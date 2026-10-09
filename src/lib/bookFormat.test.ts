import { describe, expect, it } from "vitest";
import { getBookFormat, getPathExtension } from "./bookFormat";

describe("getBookFormat", () => {
  it("распознаёт форматы по расширению", () => {
    expect(getBookFormat("a/b/Книга.PDF")).toBe("pdf");
    expect(getBookFormat("x.epub")).toBe("epub");
    expect(getBookFormat("x.fb2")).toBe("fb2");
    expect(getBookFormat("theme.typ")).toBe("typst");
    expect(getBookFormat("notes.txt")).toBeNull();
  });

  it("считает .fb2.zip обычным FB2 (бэкенд распаковывает при чтении)", () => {
    expect(getBookFormat("Толстой — Война и мир.fb2.zip")).toBe("fb2");
    expect(getBookFormat("dir\\Book.FB2.ZIP")).toBe("fb2");
    expect(getBookFormat("archive.zip")).toBeNull();
  });

  it("берёт расширение только у последнего сегмента пути", () => {
    expect(getPathExtension("v1.2/readme")).toBe("");
    expect(getPathExtension(".hidden")).toBe("");
  });
});
