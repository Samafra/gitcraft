#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use gitcraft::GitCraftApp;

#[cfg(not(target_arch = "wasm32"))]
fn load_app_icon() -> eframe::egui::IconData {
    let bytes = include_bytes!("../assets/icon.png");
    let img = image::load_from_memory(bytes)
        .expect("Error cargando assets/icon.png")
        .into_rgba8();
    let (width, height) = img.dimensions();
    eframe::egui::IconData {
        rgba: img.into_raw(),
        width,
        height,
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result<()> {
    if std::env::args().any(|arg| arg == "--mcp-server") {
        let rt = tokio::runtime::Runtime::new().expect("Error creando runtime Tokio");
        rt.block_on(gitcraft::mcp_server::run_mcp_server());
        return Ok(());
    }

    env_logger::init();
    let opts = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 780.0])
            .with_title("GitCraft")
            .with_icon(load_app_icon()),
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };
    eframe::run_native("GitCraft", opts, Box::new(|cc| Ok(Box::new(GitCraftApp::new(cc)))))
}

#[cfg(target_arch = "wasm32")]
fn main() {
    use eframe::wasm_bindgen::JsCast as _;
    wasm_bindgen_futures::spawn_local(async {
        let canvas = web_sys::window().unwrap().document().unwrap().get_element_by_id("gitcraft_canvas").unwrap().dyn_into::<web_sys::HtmlCanvasElement>().unwrap();
        eframe::WebRunner::new().start(canvas, eframe::WebOptions::default(), Box::new(|cc| Ok(Box::new(GitCraftApp::new(cc))))).await.ok();
    });
}
