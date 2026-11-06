//! Shared edit-free Line construction for typed commands and viewport getters.
use super::*;
mod input;
pub mod normal;
pub mod perpendicular;
use crate::native_length;
#[path = "../../../third_party/opennurbs_rust/line_rotation.rs"]
mod native_rotation;
pub use input::{Definition, Input, Mode};
pub const USAGE: &str = "Line [BothSides] [Vertical|FourPoint|Bisector|Angled|Extension|Normal|Perpendicular] points [angle|length]";

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
        if perpendicular::requested(args) {
            return perpendicular::run(doc, args, context);
        }
        let mut input = Input::new(context.construction_plane);
        let mut i = 0;
        let mut result = None;
        let mut normal_reference = None;
        while i < args.len() {
            if result.is_some() {
                return Err(CommandError::Usage(USAGE));
            }
            if input.option(args[i])? {
                i += 1;
                continue;
            }
            if args[i]
                .trim_start_matches('_')
                .eq_ignore_ascii_case("Enter")
            {
                result = Some(input.enter().ok_or(CommandError::Usage(USAGE))?);
                i += 1;
                continue;
            }
            if input.wants_normal_origin() {
                if let Some(reference) = &normal_reference {
                    let reference: &normal::Reference = reference;
                    let (point, n) = parse_point(&args[i..])?;
                    input.normal(reference.location(point, input.ignore_trims())?)?;
                    i += n;
                } else {
                    let (id, component) = normal::parse_binding(args[i]).ok_or(CommandError::Usage("Line Normal Curve=id|Surface=id|Face=id,index|Edge=id,index selection-point start endpoint|length"))?;
                    let (pick, n) = parse_point(&args[i + 1..])?;
                    normal_reference = Some(normal::Reference::capture(
                        doc,
                        id,
                        component,
                        pick,
                        input.ignore_trims(),
                    )?);
                    input.observe_cursor(pick);
                    i += 1 + n;
                }
                continue;
            }
            if input.wants_curve() {
                let (key, value) =
                    args[i]
                        .trim_start_matches('_')
                        .split_once('=')
                        .ok_or(CommandError::Usage(
                            "Line Extension Curve=id|Edge=id,index pick endpoint|length",
                        ))?;
                let (id, edge) = if key.eq_ignore_ascii_case("Edge") {
                    let (id, edge) = crate::curve_reference::parse_edge(value)
                        .ok_or(CommandError::Usage("Edge=id,index"))?;
                    (id, Some(edge))
                } else if key.eq_ignore_ascii_case("Curve") {
                    (
                        value.parse().map_err(|_| CommandError::Usage("Curve=id"))?,
                        None,
                    )
                } else {
                    return Err(CommandError::Usage(USAGE));
                };
                let captured = crate::curve_reference::CapturedCurve::capture(doc, id, edge)?;
                let (pick, n) = parse_point(&args[i + 1..])?;
                input.extension(captured.curve_ref(), pick)?;
                i += 1 + n;
                continue;
            }
            if input.requests_angle()
                || ((input.direction()?.is_some()
                    || input.mode() == Mode::Normal && input.endpoint())
                    && args.len() - i < 3
                    && !args[i].contains(','))
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
        if matches!(input.mode(), Mode::Extension | Mode::Normal) {
            doc.clear_selection();
        }
        let id = doc.add_geometry(Geometry::Line(definition.line))?;
        Ok(format!("Added line {id} (length {length:.6})"))
    }
}
