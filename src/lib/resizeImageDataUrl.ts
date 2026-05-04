/** Ужимает картинку для хранения в метаданных (data URL JPEG). */
export function resizeImageFileToJpegDataUrl(
  file: File,
  maxEdge = 1400,
  quality = 0.82,
): Promise<string> {
  return new Promise((resolve, reject) => {
    const url = URL.createObjectURL(file);
    const img = new Image();
    img.onload = () => {
      URL.revokeObjectURL(url);
      try {
        let { width: w, height: h } = img;
        if (w <= 0 || h <= 0) {
          reject(new Error("Некорректное изображение"));
          return;
        }
        const scale = Math.min(1, maxEdge / Math.max(w, h));
        w = Math.round(w * scale);
        h = Math.round(h * scale);
        const canvas = document.createElement("canvas");
        canvas.width = w;
        canvas.height = h;
        const ctx = canvas.getContext("2d");
        if (!ctx) {
          reject(new Error("Canvas недоступен"));
          return;
        }
        ctx.drawImage(img, 0, 0, w, h);
        resolve(canvas.toDataURL("image/jpeg", quality));
      } catch (e) {
        reject(e instanceof Error ? e : new Error(String(e)));
      }
    };
    img.onerror = () => {
      URL.revokeObjectURL(url);
      reject(new Error("Не удалось загрузить файл"));
    };
    img.src = url;
  });
}
