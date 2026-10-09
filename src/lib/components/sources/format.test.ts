import { describe, expect, it } from "vitest";
import { formatDate, formatSize, pluralBooks } from "./format";

describe("подписи каталога", () => {
  it("склоняет «книга»", () => {
    expect([1, 2, 5, 11, 12, 21, 22, 25, 111].map(pluralBooks)).toEqual([
      "1 книга",
      "2 книги",
      "5 книг",
      "11 книг",
      "12 книг",
      "21 книга",
      "22 книги",
      "25 книг",
      "111 книг",
    ]);
  });

  it("форматирует размеры и пустые значения", () => {
    expect(formatSize(0)).toBe("—");
    expect(formatSize(null)).toBe("—");
    expect(formatSize(1536)).toBe("1.5 КБ");
    expect(formatSize(5 * 1024 * 1024)).toBe("5.0 МБ");
    expect(formatDate(null)).toBe("—");
    expect(formatDate("не дата")).toBe("—");
  });
});
