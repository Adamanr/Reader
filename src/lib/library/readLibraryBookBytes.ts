import { invoke } from "@tauri-apps/api/core";

const PDF_SIG = [0x25, 0x50, 0x44, 0x46] as const; // %PDF

function hasPdfSignatureInPrefix(bytes: Uint8Array, maxScan: number): boolean {
  const n = Math.min(bytes.length, maxScan);
  outer: for (let i = 0; i <= n - 4; i++) {
    for (let j = 0; j < 4; j++) {
      if (bytes[i + j] !== PDF_SIG[j]) continue outer;
    }
    return true;
  }
  return false;
}

/**
 * Проверяет, что буфер похож на PDF (ищет %PDF в первых maxScan байтах, как при разборе).
 */
export function assertBufferLooksLikePdf(bytes: Uint8Array, context = ""): void {
  const p = context ? `${context}: ` : "";
  if (bytes.length === 0) {
    throw new Error(
      `${p}файл книги пуст (0 байт) или не прочитан. Проверьте исходный PDF в папке библиотеки.`,
    );
  }
  if (!hasPdfSignatureInPrefix(bytes, 64 * 1024)) {
    throw new Error(
      `${p}файл не похож на PDF (нет сигнатуры %PDF). Проверьте путь к книге и что это не пустой/чужой файл.`,
    );
  }
}

/** Сырые байты файла книги из папки библиотеки (Tauri). */
export async function readLibraryBookBytes(relativePath: string): Promise<Uint8Array> {
  const b64 = await invoke<string>("read_book_base64", { relativePath });
  const bin = atob(b64);
  const len = bin.length;
  const bytes = new Uint8Array(len);
  for (let i = 0; i < len; i++) bytes[i] = bin.charCodeAt(i) & 0xff;
  return bytes;
}
