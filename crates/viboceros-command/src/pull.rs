//! Closest surface pulling shares projection input, layers and atomic admission.
use super::*;
pub(super) const USAGE: &str = "Pull [Sources=id,...] [Targets=id,...] [InfinitePlane=definition] [SubCrv=id,start,end] [SubCrvLength=id,start,length] [DeleteInput=Yes|No] [OutputLayer=Current|Input|TargetObject] [Loose=No]";
pub(super) struct PullCommand;
impl Command for PullCommand {
    fn name(&self) -> &'static str {
        "Pull"
    }
    fn run(&self, d: &mut Document, a: &[&str]) -> Result<String, CommandError> {
        self.run_in_context(d, a, CommandContext::default())
    }
    fn run_in_context(
        &self,
        d: &mut Document,
        a: &[&str],
        c: CommandContext,
    ) -> Result<String, CommandError> {
        project::run_projection(d, a, c, true)
    }
}
pub(super) fn geometry(
    source: &Geometry,
    target: &Geometry,
    index: Option<&viboceros_geometry::MeshProjectionIndex<'_>>,
    tolerance: Tolerance,
) -> Result<Vec<Geometry>, CommandError> {
    if let Geometry::Point(point) = source {
        let candidate = match target {
            Geometry::NurbsSurface(s) => {
                let (u, v) = s.closest_parameters(*point, tolerance)?;
                Some(s.evaluate(u, v)?)
            }
            Geometry::Brep(b) => {
                let mut best = None;
                for face in 0..b.faces().len() {
                    if let Some((u, v)) = b.closest_parameters_on_face(face, *point, tolerance)? {
                        let candidate = b.faces()[face].surface().evaluate(u, v)?;
                        if best.is_none_or(|p| point.compare_distances(candidate, p).is_lt()) {
                            best = Some(candidate);
                        }
                    }
                }
                best
            }
            Geometry::Mesh(_) => index
                .ok_or(CommandError::Usage(USAGE))?
                .closest_point(*point)?,
            _ => return Err(CommandError::Usage(USAGE)),
        };
        return Ok(candidate.into_iter().map(Geometry::Point).collect());
    }
    let curve = source
        .nurbs_curve_representation()?
        .ok_or(CommandError::Usage(USAGE))?;
    if let Geometry::Mesh(_) = target {
        // Preserve supplied polyline corners exactly; other curves use the
        // geometry crate's bounded approximation before closest-path tracing.
        let index = index.ok_or(CommandError::Usage(USAGE))?;
        let output = if let Geometry::Polyline(polyline) = source {
            index.pull_polyline(polyline, tolerance)?
        } else {
            index.pull_curve(&curve, tolerance)?
        };
        return Ok(output.into_iter().map(Geometry::Polyline).collect());
    }
    let faces = match target {
        Geometry::NurbsSurface(s) => vec![Brep::try_surface_face(s.clone(), tolerance)?],
        Geometry::Brep(b) => (0..b.faces().len())
            .map(|i| b.sub_brep(&[i], tolerance))
            .collect::<Result<Vec<_>, _>>()?,
        _ => return Err(CommandError::Usage(USAGE)),
    };
    let mut output = Vec::new();
    for face in faces {
        let surface = face.faces()[0].surface();
        if let Some(plane) = surface.plane(tolerance)? {
            output.extend(project::planar_output(
                &curve,
                &face,
                plane,
                plane.normal().as_vector(),
                tolerance,
                true,
            )?);
        } else {
            #[cfg(feature = "native-smlib")]
            for piece in viboceros_smlib::pull_curve_to_surface(&curve, surface, tolerance)? {
                for event in viboceros_geometry::clip_surface_curve_to_face_with_operation(
                    &piece.spatial,
                    &piece.uv,
                    &face.faces()[0],
                    tolerance,
                    &mut curve_intersections::events,
                )? {
                    output.push(match event {
                        viboceros_geometry::SurfaceSurfaceIntersectionEvent::Curve(c) => {
                            Geometry::NurbsCurve(c)
                        }
                        viboceros_geometry::SurfaceSurfaceIntersectionEvent::Point(p) => {
                            Geometry::Point(p)
                        }
                    });
                }
            }
            #[cfg(not(feature = "native-smlib"))]
            return Err(CommandError::Usage(
                "Pull to curved surfaces requires native-smlib",
            ));
        }
        if output.len() > MAX_INTERSECT_OUTPUTS {
            return Err(CommandError::TooManyIntersectOutputs {
                maximum: MAX_INTERSECT_OUTPUTS,
            });
        }
    }
    // Both seam charts may represent a single spatial curve. Only a complete
    // overlap on both curves permits one witness to be dropped.
    let mut unique = Vec::new();
    let mut comparisons = 0usize;
    'candidate: for candidate in output {
        for previous in &unique {
            comparisons += 1;
            if comparisons > MAX_INTERSECT_PAIRS {
                return Err(CommandError::TooManyIntersectPairs {
                    maximum: MAX_INTERSECT_PAIRS,
                });
            }
            if duplicate(&candidate, previous, tolerance)? {
                continue 'candidate;
            }
        }
        unique.push(candidate);
    }
    Ok(unique)
}
fn covered(
    domain: std::ops::RangeInclusive<f64>,
    mut ranges: Vec<std::ops::RangeInclusive<f64>>,
) -> bool {
    ranges.sort_by(|a, b| a.start().total_cmp(b.start()));
    let mut end = *domain.start();
    for range in ranges {
        if *range.start() > end {
            return false;
        }
        end = end.max(*range.end());
    }
    end >= *domain.end()
}
fn duplicate(a: &Geometry, b: &Geometry, tolerance: Tolerance) -> Result<bool, CommandError> {
    if let (Geometry::Point(a), Geometry::Point(b)) = (a, b) {
        return Ok(a.distance_bound_to(*b, tolerance.absolute())?.is_some());
    }
    let (Some(first), Some(second)) = (a.curve_ref(), b.curve_ref()) else {
        return Ok(false);
    };
    let near = |a: Point3, b: Point3| -> Result<bool, GeometryError> {
        Ok(a.distance_bound_to(b, tolerance.absolute())?.is_some())
    };
    let a0 = first.start_point()?;
    let a1 = first.end_point()?;
    let b0 = second.start_point()?;
    let b1 = second.end_point()?;
    if !(near(a0, b0)? && near(a1, b1)? || near(a0, b1)? && near(a1, b0)?) {
        return Ok(false);
    }
    let (Some(a), Some(b)) = (
        a.nurbs_curve_representation()?,
        b.nurbs_curve_representation()?,
    ) else {
        return Ok(false);
    };
    let events = curve_intersections::events(&a, &b, tolerance)?;
    let mut first = Vec::new();
    let mut second = Vec::new();
    for event in events {
        if let CurveCurveIntersectionEvent::Overlap(overlap) = event {
            first.push(overlap.first_interval());
            second.push(overlap.second_interval());
        }
    }
    Ok(covered(a.domain(), first) && covered(b.domain(), second))
}
#[cfg(test)]
mod tests;
