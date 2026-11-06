//! Command polylines retain physical chord parameters, unlike SDK unit-span defaults.
use super::*;
#[path = "../../../third_party/opennurbs_rust/length.rs"]
mod native_length;
pub(super) struct PolylineCommand;

impl Command for PolylineCommand {
    fn name(&self) -> &'static str {
        "Polyline"
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["PLine"]
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let mut vertices = Vec::new();
        let mut consumed = 0;
        while consumed < arguments.len() {
            let (vertex, vertex_consumed) = parse_point(&arguments[consumed..])?;
            vertices.push(vertex);
            consumed += vertex_consumed;
        }
        let mut parameters = Vec::with_capacity(vertices.len());
        parameters.push(0.);
        for pair in vertices.windows(2) {
            let d = pair[0].vector_to(pair[1])?;
            parameters.push(parameters.last().unwrap() + native_length::length3d(d.to_array()));
        }
        let polyline = Polyline3::try_with_parameters(vertices, parameters, document.tolerance())?;
        let vertex_count = polyline.vertices().len();
        let segment_count = polyline.segment_count();
        let closed = polyline.is_closed();
        let id = document.add_geometry(Geometry::Polyline(polyline))?;
        Ok(format!(
            "Added {}polyline {id} ({vertex_count} vertices, {segment_count} segments)",
            if closed { "closed " } else { "" }
        ))
    }
}
