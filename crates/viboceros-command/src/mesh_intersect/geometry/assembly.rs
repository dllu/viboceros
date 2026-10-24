//! Assemble line intersections independently of coplanar area contours.
use super::*;
use viboceros_geometry::{CurveJoinStyle, Polyline3};
mod coverage;

pub(super) fn assemble(
    mut segments: Vec<[Point3; 2]>,
    contacts: Vec<[Point3; 2]>,
    mut contours: Vec<Polyline3>,
    tolerance: Tolerance,
) -> Result<Vec<Polyline3>, CommandError> {
    segments.extend(coverage::subtract(contacts, &contours, tolerance)?);
    let lines = joined_lines(segments, tolerance)?;
    let bounds = |p: &Polyline3| [p.bounds().max().to_array(), p.bounds().min().to_array()];
    if lines.is_empty() {
        contours.sort_by(|a, b| {
            bounds(b)
                .partial_cmp(&bounds(a))
                .expect("finite contour bounds")
        });
    }
    contours.extend(lines);
    Ok(contours)
}

fn joined_lines(
    mut segments: Vec<[Point3; 2]>,
    tolerance: Tolerance,
) -> Result<Vec<Polyline3>, CommandError> {
    let compare = |a: Point3, b: Point3| {
        a.to_array()
            .partial_cmp(&b.to_array())
            .expect("finite points")
    };
    for segment in &mut segments {
        if compare(segment[0], segment[1]).is_gt() {
            segment.swap(0, 1);
        }
    }
    segments.sort_by(|a, b| compare(a[0], b[0]).then_with(|| compare(a[1], b[1])));
    segments.dedup();
    let curves = segments
        .into_iter()
        .filter(|p| p[0] != p[1])
        .map(|[a, b]| LineSegment::try_new(a, b, Tolerance::MESH_VALIDATION).map(Curve3::Line))
        .collect::<Result<Vec<_>, _>>()?;
    let options = CurveJoinOptions {
        tolerance: tolerance.absolute(),
        preserve_direction: false,
        style: CurveJoinStyle::Batch,
    };
    let polylines = join_curves(&curves, options, Tolerance::MESH_VALIDATION)?
        .into_iter()
        .map(|joined| {
            Ok(match joined.into_curve() {
                Curve3::Polyline(p) => {
                    Polyline3::try_new(p.vertices().to_vec(), Tolerance::MESH_VALIDATION)?
                }
                Curve3::Line(l) => {
                    Polyline3::try_new(vec![l.start(), l.end()], Tolerance::MESH_VALIDATION)?
                }
                _ => {
                    return Err(CommandError::Usage(
                        "mesh intersection returned a non-polyline",
                    ));
                }
            })
        })
        .collect::<Result<Vec<_>, CommandError>>()?;
    Ok(polylines)
}
