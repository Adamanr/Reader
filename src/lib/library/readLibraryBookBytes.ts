import { invoke } from "@tauri-apps/api/core";

const PDF_SIG = [0x25, 0x50, 0x44, 0x46] as const; // %PDF
const BASE64_CHUNK_SIZE = 1024 * 1024;

function yieldToUi(): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, 0));
}

/** Декодирует большие файлы порциями, не блокируя WebView на десятки кадров. */
async function decodeBase64InChunks(base64: string): Promise<Uint8Array> {
  const padding = base64.endsWith("==") ? 2 : base64.endsWith("=") ? 1 : 0;
  const outputLength = Math.floor((base64.length * 3) / 4) - padding;
  const bytes = new Uint8Array(outputLength);
  let offset = 0;

  for (let start = 0; start < base64.length; start += BASE64_CHUNK_SIZE) {
    const binary = atob(base64.slice(start, start + BASE64_CHUNK_SIZE));
    for (let index = 0; index < binary.length; index++) {
      bytes[offset++] = binary.charCodeAt(index) & 0xff;
    }
    if (start + BASE64_CHUNK_SIZE < base64.length) await yieldToUi();
  }

  return bytes;
}

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
  return decodeBase64InChunks(b64);
}
