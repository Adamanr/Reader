// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // WebKitGTK на проприетарном драйвере NVIDIA с DMA-BUF рендерером
    // теряет фрагменты отрисовки (в том числе текст страниц PDF).
    // Пользователь может переопределить это своей переменной окружения.
    #[cfg(target_os = "linux")]
    if std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none()
        && std::path::Path::new("/proc/driver/nvidia/version").exists()
    {
        std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
    }
    reader_lib::run()
}
