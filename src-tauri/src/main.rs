// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // DMA-BUF рендерер WebKitGTK намеренно не отключаем: без него на NVIDIA
    // кадры копируются через процессор и прокрутка заметно тормозит.
    // Пропадание текста PDF лечится иначе — страницы рисуются на CPU-холсте.
    // При артефактах отрисовки можно запустить с WEBKIT_DISABLE_DMABUF_RENDERER=1.
    reader_lib::run()
}
