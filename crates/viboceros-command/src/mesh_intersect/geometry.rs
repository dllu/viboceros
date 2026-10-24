//! Kernel imprint segments, unique-edge admission, and shared curve joining.
use super::*;
use viboceros_geometry::{CurveJoinStyle, Polyline3};

pub(super) fn compute(
    meshes: &[&TriangleMesh],
    options: Options,
    document: Tolerance,
) -> Result<Vec<Polyline3>, CommandError> {
    let tolerance = options.geometry_tolerance(document);
    let mut segments = Vec::new();
    let mut only_coplanar = true;
    for (i, a) in meshes.iter().enumerate() {
        for b in &meshes[i + 1..] {
            let result = viboceros_smlib::mesh_intersections(a, b, tolerance)?;
            only_coplanar &= result.transverse.is_empty();
            segments.extend(result.transverse);
            if options.coplanar == "Polylines" {
                segments.extend(result.coplanar);
            }
            if segments.len() > MAX_SPAN_OUTPUT_OBJECTS {
                return Err(too_many_span_outputs("mesh intersection"));
            }
        }
    }
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
    let mut polylines = join_curves(&curves, options, Tolerance::MESH_VALIDATION)?
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
    // Order disconnected coplanar contours by descending bounds, retaining
    // outer boundaries before holes after joining their constituent segments.
    if only_coplanar {
        let bounds = |p: &Polyline3| [p.bounds().max().to_array(), p.bounds().min().to_array()];
        polylines.sort_by(|a, b| {
            bounds(b)
                .partial_cmp(&bounds(a))
                .expect("finite contour bounds")
        });
    }
    Ok(polylines)
}
