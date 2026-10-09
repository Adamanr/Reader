import { describe, expect, it } from "vitest";
import { opdsSearchUrl } from "./opds";

describe("opdsSearchUrl", () => {
  it("подставляет запрос и убирает необязательные параметры OpenSearch", () => {
    expect(opdsSearchUrl("https://x.org/s?q={searchTerms}&p={startPage?}", " Война и мир ")).toBe(
      "https://x.org/s?q=%D0%92%D0%BE%D0%B9%D0%BD%D0%B0%20%D0%B8%20%D0%BC%D0%B8%D1%80&p=",
    );
  });

  it("экранирует спецсимволы запроса", () => {
    expect(opdsSearchUrl("https://x.org/s/{searchTerms}", "a&b/c")).toBe("https://x.org/s/a%26b%2Fc");
  });
});
