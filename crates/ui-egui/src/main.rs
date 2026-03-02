use eframe::{egui, NativeOptions, Renderer};

mod app;

fn main() -> Result<(), eframe::Error> {
    env_logger::init();

    let options = NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 720.0])
            .with_title("CadKit - 2D CAD Platform"),
        renderer: Renderer::Wgpu,
        ..Default::default()
    };

    eframe::run_native(
        "CadKit",
        options,
        Box::new(|cc| Box::new(app::CadKitApp::new(cc))),
    )
}
