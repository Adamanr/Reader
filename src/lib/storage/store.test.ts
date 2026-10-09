import { describe, expect, it } from "vitest";
import { pathKey } from "./store";

describe("pathKey", () => {
  // Бэкенд переносит файлы `glossary-…`, `recap-…`, `epubloc-…` при переименовании книги
  // и вычисляет этот ключ сам (`front_path_key` в src-tauri/src/library.rs) — они должны совпадать.
  it("совпадает с ключом, который считает Rust", async () => {
    const full = Array.from(
      new Uint8Array(await crypto.subtle.digest("SHA-256", new TextEncoder().encode("a.pdf"))),
    )
      .map((b) => b.toString(16).padStart(2, "0"))
      .join("");
    expect(await pathKey("a.pdf")).toBe(full.slice(0, 24));
    expect(await pathKey("a.pdf")).toHaveLength(24);
  });
});
