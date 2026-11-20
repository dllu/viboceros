use super::*;
use crate::{Circle3, LineSegment, Polyline3, SurfaceIso};

fn point(x: Real, y: Real, z: Real) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}

#[test]
fn sweep_miters_join_a_rational_circle_without_sampling_the_trim() {
    let tolerance = Tolerance::try_new(1e-7, 1e-12, 1e-10).unwrap();
    let path = Polyline3::try_new(
        vec![point(0., 0., 0.), point(5., 0., 0.), point(5., 5., 0.)],
        tolerance,
    )
    .unwrap();
    let origin = point(0., 0., 0.);
    let normal = Vector3::try_new(1., 0., 0.)
        .unwrap()
        .normalized_nonzero()
        .unwrap();
    let profile = Circle3::try_from_center_point(origin, point(0., 1.5, 0.), normal, tolerance)
        .unwrap()
        .to_nurbs()
        .unwrap();
    let guide = CurveRef::Polyline(&path);
    for untrimmed in [false, true] {
        let section = SweepSection {
            parameter: *guide.domain().start(),
            curve: profile.clone(),
        };
        let brep = Sweep1::try_polyline_miters(
            guide,
            &section,
            guide.domain(),
            SweepFrameStyle::Freeform,
            untrimmed,
            tolerance,
        )
        .unwrap()
        .unwrap();
        let (faces, edges, rings) = if untrimmed { (2, 5, 1) } else { (4, 10, 4) };
        assert_eq!((brep.faces().len(), brep.edges().len()), (faces, edges));
        let in_miter = |p: Point3| {
            (p.x() + p.y() - 5.).abs() < 1e-9 && (p.y().hypot(p.z()) - 1.5).abs() < 1e-9
        };
        let middle = brep
            .edges()
            .iter()
            .filter(|edge| {
                let curve = edge.curve();
                in_miter(curve.evaluate(curve.parameter_at(0.5).unwrap()).unwrap())
            })
            .collect::<Vec<_>>();
        assert_eq!(middle.len(), rings);
        for edge in middle {
            for index in 0..=32 {
                let t = edge.curve().parameter_at(index as Real / 32.).unwrap();
                assert!(in_miter(edge.curve().evaluate(t).unwrap()));
            }
        }
        let non_iso = brep
            .faces()
            .iter()
            .flat_map(|face| face.loops())
            .flat_map(|l| l.trims())
            .any(|trim| trim.iso() == SurfaceIso::NotIso);
        assert_eq!(non_iso, !untrimmed);
    }
}

#[test]
fn sweep_miters_split_exterior_extensions_at_the_original_rail_ends() {
    let tolerance = Tolerance::try_new(1e-7, 1e-12, 1e-10).unwrap();
    let rail = Polyline3::try_new(
        vec![point(0., 0., 0.), point(5., 0., 0.), point(5., 5., 0.)],
        tolerance,
    )
    .unwrap();
    let guide = CurveRef::Polyline(&rail);
    for (start, end, native_area, vertices, edges) in [
        (
            point(0., 0., 0.),
            point(0., -1.5, 1.),
            20.73191983416793,
            8,
            11,
        ),
        (
            point(0., -1., 0.),
            point(0., -2., 1.),
            18.38477631085024,
            10,
            13,
        ),
    ] {
        let section = SweepSection {
            parameter: *guide.domain().start(),
            curve: LineSegment::try_new(start, end, tolerance)
                .unwrap()
                .to_nurbs()
                .unwrap(),
        };
        let brep = Sweep1::try_polyline_miters(
            guide,
            &section,
            guide.domain(),
            SweepFrameStyle::Freeform,
            false,
            tolerance,
        )
        .unwrap()
        .unwrap();
        assert_eq!(
            (
                brep.faces().len(),
                brep.vertices().len(),
                brep.edges().len()
            ),
            (4, vertices, edges)
        );
        let natural_faces = brep
            .faces()
            .iter()
            .filter(|face| {
                face.loops()[0]
                    .trims()
                    .iter()
                    .all(|trim| trim.iso() != SurfaceIso::NotIso)
            })
            .count();
        assert_eq!(natural_faces, 2);
        assert!((brep.area(tolerance).unwrap() - native_area).abs() < 1e-7);
    }
}

#[test]
fn sweep_miter_extensions_partition_between_rational_profile_knots() {
    let tolerance = Tolerance::try_new(1e-7, 1e-12, 1e-10).unwrap();
    let rail = Polyline3::try_new(
        vec![point(0., 0., 0.), point(5., 0., 0.), point(5., 5., 0.)],
        tolerance,
    )
    .unwrap();
    let guide = CurveRef::Polyline(&rail);
    let normal = Vector3::try_new(1., 0., 0.)
        .unwrap()
        .normalized_nonzero()
        .unwrap();
    // Rhino 8.32 with UntrimmedMiters=No. A translated circle and a rotated
    // seam both put the extension crossings between the authored V knots.
    for (center, radius, phase, topology) in [
        (0., 1.5, std::f64::consts::FRAC_PI_6, (4, 8, 12)),
        (0.35, 1.5, 0., (4, 8, 12)),
        (-0.35, 1.5, 0., (4, 8, 12)),
        (0., 1.5, std::f64::consts::PI, (6, 8, 14)),
        (-3., 0.75, 0., (4, 8, 12)),
        (3., 0.75, 0., (2, 6, 8)),
    ] {
        let (sine, cosine) = phase.sin_cos();
        let curve = Circle3::try_from_center_point(
            point(0., center, 0.),
            point(0., center + radius * cosine, radius * sine),
            normal,
            tolerance,
        )
        .unwrap()
        .to_nurbs()
        .unwrap();
        let section = SweepSection {
            parameter: *guide.domain().start(),
            curve,
        };
        let brep = Sweep1::try_polyline_miters(
            guide,
            &section,
            guide.domain(),
            SweepFrameStyle::Freeform,
            false,
            tolerance,
        )
        .unwrap()
        .unwrap();
        assert_eq!(
            (
                brep.faces().len(),
                brep.vertices().len(),
                brep.edges().len()
            ),
            topology,
            "center {center}, phase {phase}"
        );
        assert_eq!(
            brep.edge_use_counts()
                .into_iter()
                .filter(|&n| n == 1)
                .count(),
            2
        );
        if center.abs() < 1. {
            let z = (radius * radius - center * center).sqrt();
            for z in [-z, z] {
                assert!(
                    brep.vertices().iter().any(|v| v
                        .point()
                        .distance_to(point(5., 0., z))
                        .unwrap()
                        < 1e-8)
                );
            }
        }
    }
}

#[test]
fn sweep_miter_sewing_matches_native_spatial_and_closed_polyline_areas() {
    let tolerance = Tolerance::try_new(1e-7, 1e-12, 1e-10).unwrap();
    for (points, native_area, faces, edges) in [
        (
            vec![
                point(0., 0., 0.),
                point(5., 0., 0.),
                point(5., 5., 0.),
                point(5., 5., 5.),
            ],
            22.53469547164993,
            3,
            10,
        ),
        (
            vec![
                point(0., 0., 0.),
                point(5., 0., 0.),
                point(5., 5., 0.),
                point(0., 5., 0.),
                point(0., 0., 0.),
            ],
            25.238858928247906,
            4,
            12,
        ),
    ] {
        let rail = Polyline3::try_new(points, tolerance).unwrap();
        let guide = CurveRef::Polyline(&rail);
        let section = SweepSection {
            parameter: *guide.domain().start(),
            curve: LineSegment::try_new(point(0., 0., 0.), point(0., 1.5, 1.), tolerance)
                .unwrap()
                .to_nurbs()
                .unwrap(),
        };
        let brep = Sweep1::try_polyline_miters(
            guide,
            &section,
            guide.domain(),
            SweepFrameStyle::Freeform,
            false,
            tolerance,
        )
        .unwrap()
        .unwrap();
        assert_eq!((brep.faces().len(), brep.edges().len()), (faces, edges));
        let area = brep.area(tolerance).unwrap();
        assert!((area - native_area).abs() < 1e-7, "{area}");
    }
}
