//! CreateUVCrv flattens one surface chart, trim loops, curves and points.
use super::*;
use viboceros_geometry::{BrepLoopType, NurbsCurve2, Point2, PolyCurve3, remap_scalar};
mod projection;
#[cfg(test)]
mod tests;

pub(super) const USAGE: &str =
    "CreateUVCrv Surface=surface-uuid [Face=index] (optionally select curves/points on surface)";
pub(super) struct CreateUvCurvesCommand;

impl Command for CreateUvCurvesCommand {
    fn name(&self) -> &'static str {
        "CreateUVCrv"
    }
    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let reference = uv_reference::resolve(document, arguments, USAGE)?;
        let target = reference.object;
        let surface = reference.surface;
        let face = reference.face;
        let tolerance = document.tolerance();
        let size = rectangle_size(surface, tolerance)?;
        let chart = Chart { surface, size };
        let mut staged = Vec::new();
        let rectangle = rectangle(surface)?;
        let mut natural_outer = false;
        if let Some(face) = face {
            for boundary in face.loops() {
                let curves = boundary
                    .trims()
                    .iter()
                    .map(|t| t.curve().clone())
                    .collect::<Vec<_>>();
                // Suppress the outer loop only when it is the entire natural
                // rectangle. Inner loops and trimmed outer loops remain output.
                let is_natural = boundary.loop_type() == BrepLoopType::Outer
                    && natural_loop(&curves, &rectangle)?;
                if is_natural {
                    natural_outer = true;
                    staged.push((target, chart.loop_curve(&rectangle, tolerance)?));
                } else {
                    staged.push((target, chart.loop_curve(&curves, tolerance)?));
                }
            }
        }
        if !natural_outer {
            staged.push((target, chart.loop_curve(&rectangle, tolerance)?));
        }
        let sources = document
            .selected_objects()
            .filter(|o| {
                o.id() != target
                    && (o.geometry().curve_ref().is_some()
                        || matches!(o.geometry(), Geometry::Point(_)))
            })
            .map(|o| (o.id(), o.geometry()))
            .collect::<Vec<_>>();
        let source_ids = sources.iter().map(|(id, _)| *id).collect::<Vec<_>>();
        let numerical = Tolerance::try_new(
            (tolerance.absolute() * 1e-4).max(Real::MIN_POSITIVE),
            Real::EPSILON * 8.,
            tolerance.angular(),
        )?;
        let mut distant = 0;
        for (id, geometry) in sources {
            if let Geometry::Point(p) = geometry {
                let (u, v) = surface.closest_parameters(*p, numerical)?;
                if surface.evaluate(u, v)?.distance_to(*p)? > tolerance.absolute() * 2. {
                    distant += 1;
                }
                staged.push((id, Geometry::Point(chart.point(Point2::try_new(u, v)?)?)));
            } else {
                let source = geometry.nurbs_curve_representation()?.unwrap();
                match surface.try_pullback_curve_certified(&source, tolerance) {
                    Ok(uv) => staged.push((id, Geometry::NurbsCurve(chart.curve(&uv)?))),
                    Err(
                        GeometryError::SurfacePullbackDidNotConverge { .. }
                        | GeometryError::Degenerate { .. }
                        | GeometryError::InvalidControlNet {
                            context: "curve must lie on the surface for parameter-space pullback",
                        },
                    ) => {
                        staged.push((
                            id,
                            Geometry::NurbsCurve(projection::fit(
                                &chart, &source, tolerance, numerical,
                            )?),
                        ));
                        distant += 1;
                    }
                    Err(error) => return Err(error.into()),
                }
            }
        }
        document.release_command_selection_on_history_replay(
            source_ids.iter().copied().chain([target]),
        )?;
        // Multiple surface border outputs share one recreated source group,
        // while extra objects participate in the same source-group remapping.
        let outputs = document.copy_object_pieces_into_source_groups(staged)?;
        let mut group_map = BTreeMap::new();
        for &id in &outputs {
            let memberships = document.object(id).unwrap().group_ids().to_vec();
            let mut copies = Vec::new();
            for group in memberships {
                let new_group = if let Some(&copy) = group_map.get(&group) {
                    copy
                } else {
                    let copy = document.add_empty_group(Some(document.next_unused_group_name()))?;
                    group_map.insert(group, copy);
                    copy
                };
                copies.push(new_group);
            }
            document.set_object_group_memberships(id, copies)?;
        }
        document.retain_created_group_definitions_on_undo()?;
        document.select_command_results(outputs.iter().copied().chain(source_ids))?;
        Ok(format!(
            "Created {} UV curve(s)/point(s); {distant} input(s) farther than surface tolerance",
            outputs.len()
        ))
    }
}

fn rectangle_size(
    surface: &NurbsSurface,
    tolerance: Tolerance,
) -> Result<[Real; 2], GeometryError> {
    let mut size = [0_f64; 2];
    // Public-command observations use eight interior stations, excluding the
    // natural end. Use accuracy-controlled length rather than Rhino's coarse
    // fractional integration; retained native sizing discrepancies are explicit.
    for i in 0..8 {
        let t = i as Real / 8.;
        size[0] = size[0].max(
            surface
                .isocurve_u(surface.parameter_at_v(t)?)?
                .length(tolerance)?,
        );
        size[1] = size[1].max(
            surface
                .isocurve_v(surface.parameter_at_u(t)?)?
                .length(tolerance)?,
        );
    }
    if size.iter().any(|x| *x <= 0.) {
        return Err(GeometryError::Degenerate {
            context: "UV rectangle size",
        });
    }
    Ok(size)
}

fn rectangle(surface: &NurbsSurface) -> Result<Vec<NurbsCurve2>, GeometryError> {
    let u = surface.domain_u();
    let v = surface.domain_v();
    let points = [
        Point2::try_new(*u.start(), *v.start())?,
        Point2::try_new(*u.end(), *v.start())?,
        Point2::try_new(*u.end(), *v.end())?,
        Point2::try_new(*u.start(), *v.end())?,
    ];
    (0..4)
        .map(|i| {
            NurbsCurve2::try_new(
                1,
                vec![points[i], points[(i + 1) % 4]],
                vec![if i % 2 == 0 { *u.start() } else { *v.start() }; 2]
                    .into_iter()
                    .chain(vec![if i % 2 == 0 { *u.end() } else { *v.end() }; 2])
                    .collect(),
            )
        })
        .collect()
}

fn natural_loop(curves: &[NurbsCurve2], rectangle: &[NurbsCurve2]) -> Result<bool, GeometryError> {
    if curves.len() != 4
        || curves
            .iter()
            .any(|c| c.degree() != 1 || c.control_points().len() != 2)
    {
        return Ok(false);
    }
    for offset in 0..4 {
        if (0..4).all(|i| {
            curves[i]
                .control_points()
                .iter()
                .map(|p| p.point())
                .eq(rectangle[(i + offset) % 4]
                    .control_points()
                    .iter()
                    .map(|p| p.point()))
        }) {
            return Ok(true);
        }
    }
    Ok(false)
}

struct Chart<'a> {
    surface: &'a NurbsSurface,
    size: [Real; 2],
}
impl Chart<'_> {
    fn point(&self, p: Point2) -> Result<Point3, GeometryError> {
        Point3::try_new(
            remap_scalar(
                p.x(),
                [
                    *self.surface.domain_u().start(),
                    *self.surface.domain_u().end(),
                ],
                [0., self.size[0]],
            )?,
            remap_scalar(
                p.y(),
                [
                    *self.surface.domain_v().start(),
                    *self.surface.domain_v().end(),
                ],
                [0., self.size[1]],
            )?,
            0.,
        )
    }
    fn curve(&self, curve: &NurbsCurve2) -> Result<NurbsCurve, GeometryError> {
        NurbsCurve::try_new_rational(
            curve.degree(),
            curve
                .control_points()
                .iter()
                .map(|p| WeightedPoint3::try_new(self.point(p.point())?, p.weight()))
                .collect::<Result<Vec<_>, _>>()?,
            curve.knots().to_vec(),
        )
    }
    fn loop_curve(
        &self,
        curves: &[NurbsCurve2],
        tolerance: Tolerance,
    ) -> Result<Geometry, GeometryError> {
        let curves = curves
            .iter()
            .map(|c| self.curve(c))
            .collect::<Result<Vec<_>, _>>()?;
        if curves.len() == 1 {
            return Ok(Geometry::NurbsCurve(curves[0].clone()));
        }
        let mut t = *curves[0].domain().start();
        let mut segments = Vec::new();
        for curve in curves {
            let end = t + (*curve.domain().end() - *curve.domain().start());
            segments.push(curve.try_reparameterized(t..=end)?);
            t = end;
        }
        let _ = tolerance;
        Ok(Geometry::PolyCurve(PolyCurve3::try_new(segments)?))
    }
}
