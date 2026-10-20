mod app;
#[cfg(test)]
#[allow(dead_code)]
#[path = "../crates/viboceros-command/tests/support/stretch.rs"]
mod deformation_test_support;
mod sidebar;
#[cfg(test)]
#[allow(dead_code)]
#[path = "../crates/viboceros-command/tests/support/splop.rs"]
mod splop_test_support;
#[cfg(test)]
pub(crate) use deformation_test_support::{grips as grip_fixtures, morph as morph_fixtures};
#[cfg(test)]
#[path = "../crates/viboceros-command/tests/support/splop_finish.rs"]
mod splop_finish_test_support;
#[cfg(test)]
#[allow(dead_code)]
#[path = "../crates/viboceros-command/tests/support/splop_grips.rs"]
mod splop_grips_test_support;
#[cfg(test)]
#[allow(dead_code)]
#[path = "../crates/viboceros-command/tests/support/splop_guided.rs"]
mod splop_guided_test_support;
#[cfg(test)]
#[allow(dead_code)]
#[path = "../crates/viboceros-command/tests/support/splop_reference.rs"]
mod splop_reference_test_support;
#[cfg(test)]
#[path = "../crates/viboceros-oracle/src/test_json.rs"]
mod test_json;
mod viewport;
mod viewport_gpu;

use eframe::egui;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        renderer: eframe::Renderer::Wgpu,
        depth_buffer: 32,
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
