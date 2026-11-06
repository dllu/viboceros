//! Shared edit-free Line construction for typed commands and viewport getters.
use super::*;
mod input;
use crate::native_length;
#[path = "../../../third_party/opennurbs_rust/line_rotation.rs"]
mod native_rotation;
pub use input::{Definition, Input, Mode};
pub const USAGE: &str =
    "Line [BothSides] [Vertical|FourPoint|Bisector|Angled] points [angle|length]";

/// Admission is independent of document modelling tolerance. The getter also
/// rejects coincident points before constructing a document line.
pub fn segment(start: Point3, end: Point3) -> Result<LineSegment, CommandError> {
    let line = LineSegment::try_new(start, end, Tolerance::NUMERICAL_VALIDATION)?;
    let length = native_length::length3d(start.vector_to(end)?.to_array());
    // Public ON_LineCurve constructor's tiny-length unit-domain fallback.
    let domain_end = if length <= 2.3283064365386963e-10 {
        1.0
    } else {
        length
    };
    Ok(line.try_reparameterized(0.0..=domain_end)?)
}

pub(super) struct LineCommand;
impl Command for LineCommand {
    fn name(&self) -> &'static str {
        "Line"
    }
    fn aliases(&self) -> &'static [&'static str] {
        &["L"]
    }
    fn run(&self, doc: &mut Document, args: &[&str]) -> Result<String, CommandError> {
        self.run_in_context(doc, args, CommandContext::default())
    }
    fn run_in_context(
        &self,
        doc: &mut Document,
        args: &[&str],
        context: CommandContext,
    ) -> Result<String, CommandError> {
        let mut input = Input::new(context.construction_plane);
        let mut i = 0;
        let mut result = None;
        while i < args.len() {
            if result.is_some() {
                return Err(CommandError::Usage(USAGE));
            }
            if input.option(args[i])? {
                i += 1;
                continue;
            }
            if input.requests_angle()
                || (input.direction()?.is_some() && args.len() - i < 3 && !args[i].contains(','))
            {
                result = input.number(parse_finite_real(args[i])?)?;
                i += 1;
            } else {
                let (p, n) = parse_point(&args[i..])?;
                result = input.point(p)?;
                if input.cancelled() {
                    return Err(CommandError::Usage("Line bisector has opposite rays"));
                }
                i += n;
            }
        }
        let definition = result.ok_or(CommandError::Usage(USAGE))?;
        let length = definition.line.length()?;
        let id = doc.add_geometry(Geometry::Line(definition.line))?;
        Ok(format!("Added line {id} (length {length:.6})"))
    }
}
