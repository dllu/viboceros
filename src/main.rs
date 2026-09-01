mod app;
mod sidebar;
mod viewport;

use eframe::egui;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        renderer: eframe::Renderer::Wgpu,
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1_200.0, 800.0])
            .with_min_inner_size([760.0, 480.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Viboceros",
        options,
        Box::new(|creation_context| Ok(Box::new(app::VibocerosApp::new(creation_context)))),
    )
}
