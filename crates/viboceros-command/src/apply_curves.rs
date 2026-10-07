//! ApplyCrv maps World-XY curves and points through a shared UV rectangle.
use super::*;
use viboceros_document::CopyGroupPolicy;
use viboceros_geometry::{NurbsCurve2, Point2, WeightedPoint2, remap_scalar};

#[cfg(test)]
mod tests;

pub(super) const USAGE: &str = "ApplyCrv Surface=surface-uuid (select World-XY curves and points)";
pub(super) struct ApplyCurvesCommand;

impl Command for ApplyCurvesCommand {
    fn name(&self) -> &'static str {
        "ApplyCrv"
    }
    fn aliases(&self) -> &'static [&'static str] {
        &["ApplyCurves"]
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let [argument] = arguments else {
            return Err(CommandError::Usage(USAGE));
        };
        let (name, value) = argument.split_once('=').ok_or(CommandError::Usage(USAGE))?;
        if !option_name_eq(name, "Surface") {
            return Err(CommandError::Usage(USAGE));
        }
        let target: ObjectId = value.parse().map_err(|_| CommandError::Usage(USAGE))?;
        if !document.is_object_selectable(target) {
            return Err(CommandError::Usage(USAGE));
        }
        let object = document
            .object(target)
            .ok_or(DocumentError::ObjectNotFound(target))?;
        let surface = match object.geometry() {
            Geometry::NurbsSurface(s) => s,
            Geometry::Brep(b) if b.faces().len() == 1 => b.faces()[0].surface(),
            _ => return Err(CommandError::Usage(USAGE)),
        };
        let selected = selected_ids(document)?;
        let tolerance = document.tolerance();
        let numerical = Tolerance::try_new(
            (tolerance.absolute() * 1e-4).max(Real::MIN_POSITIVE),
            Real::EPSILON * 8.,
            tolerance.angular(),
        )?;
        let mut sources = Vec::new();
        let mut bounds: Option<BoundingBox3> = None;
        let mut skipped = 0;
        for id in selected.iter().copied().filter(|id| *id != target) {
            let object = document.object(id).unwrap();
            if !matches!(object.geometry(), Geometry::Point(_))
                && object.geometry().curve_ref().is_none()
            {
                continue;
            }
            let box3 = object.geometry().tight_bounds(numerical)?;
            if box3.min().z() < -tolerance.absolute() || box3.max().z() > tolerance.absolute() {
                skipped += 1;
                continue;
            }
            bounds = Some(bounds.map_or(Ok(box3), |old| old.union(box3))?);
            sources.push((id, object.geometry()));
        }
        let Some(bounds) = bounds else {
            return Err(CommandError::Usage(USAGE));
        };
        let mut curves = Vec::new();
        let mut points = Vec::new();
        if bounds.min().x() != bounds.max().x() && bounds.min().y() != bounds.max().y() {
            let u = [*surface.domain_u().start(), *surface.domain_u().end()];
            let v = [*surface.domain_v().start(), *surface.domain_v().end()];
            let map = |p: Point3| -> Result<Point2, GeometryError> {
                Point2::try_new(
                    remap_scalar(p.x(), [bounds.min().x(), bounds.max().x()], u)?,
                    remap_scalar(p.y(), [bounds.min().y(), bounds.max().y()], v)?,
                )
            };
            for (id, geometry) in sources {
                if let Geometry::Point(p) = geometry {
                    let uv = map(*p)?;
                    points.push((id, Geometry::Point(surface.evaluate(uv.x(), uv.y())?)));
                } else {
                    let source = geometry.nurbs_curve_representation()?.unwrap();
                    let uv = NurbsCurve2::try_new_rational(
                        source.degree(),
                        source
                            .control_points()
                            .iter()
                            .map(|p| WeightedPoint2::try_new(map(p.point())?, p.weight()))
                            .collect::<Result<Vec<_>, GeometryError>>()?,
                        source.knots().to_vec(),
                    )?;
                    curves.push((
                        id,
                        Geometry::NurbsCurve(surface.try_pushup_curve_certified(&uv, tolerance)?),
                    ));
                }
            }
        }
        // Native ApplyCrv releases all source/target picking on Undo/Redo, even
        // for preselection and the successful no-output degenerate rectangle.
        document.release_command_selection_on_history_replay(
            selected.iter().copied().chain([target]),
        )?;
        let curve_count = curves.len();
        let outputs = document.copy_object_geometries_with_groups(
            curves.into_iter().chain(points),
            CopyGroupPolicy::Preserve,
        )?;
        // Native ApplyCrv allocates corresponding group definitions even for
        // point-only input, but only its curve outputs retain memberships.
        document.clear_object_group_memberships(outputs[curve_count..].iter().copied())?;
        document.retain_created_group_definitions_on_undo()?;
        if outputs.is_empty() {
            document.select_command_results(selected.into_iter().chain([target]))?;
        } else {
            document.select_command_results(outputs.iter().copied())?;
        }
        Ok(format!(
            "Applied {} curve(s)/point(s) to surface; skipped {skipped} input(s) outside World XY tolerance",
            outputs.len()
        ))
    }
}
