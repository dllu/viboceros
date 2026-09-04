use super::*;
use crate::{
    BrepEdge, BrepLoop, BrepLoopType, BrepTrim, BrepTrimType, BrepVertex, Circle3, NurbsCurve2,
    Point2, Point3, SurfaceIso, WeightedPoint2,
};

fn point(x: Real, y: Real, z: Real) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}

fn paraboloid() -> NurbsSurface {
    let coordinates = [-1.0, 0.0, 1.0];
    let squared_controls = [1.0, -1.0, 1.0];
    NurbsSurface::try_new(
        2,
        2,
        3,
        3,
        (0..3)
            .flat_map(|v| {
                (0..3).map(move |u| {
                    point(
                        coordinates[u],
                        coordinates[v],
                        squared_controls[u] + squared_controls[v],
                    )
                })
            })
            .collect(),
        vec![-1.0, -1.0, -1.0, 1.0, 1.0, 1.0],
        vec![-1.0, -1.0, -1.0, 1.0, 1.0, 1.0],
    )
    .unwrap()
}

fn round_trim(surface: NurbsSurface, radii: &[Real], capped: bool) -> Brep {
    let mut vertices = Vec::new();
    let mut edges = Vec::new();
    let mut loops = Vec::new();
    for (index, &radius) in radii.iter().enumerate() {
        let mut edge = Circle3::try_new(
            point(0.0, 0.0, radius * radius),
            radius,
            Vector3::try_new(0.0, 0.0, 1.0)
                .unwrap()
                .normalized(Tolerance::DEFAULT)
                .unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap()
        .to_nurbs()
        .unwrap();
        if index > 0 {
            edge = edge.reversed().unwrap();
        }
        let trim = NurbsCurve2::try_new_rational(
            edge.degree(),
            edge.control_points()
                .iter()
                .map(|control| {
                    WeightedPoint2::try_new(
                        Point2::try_new(control.point().x(), control.point().y()).unwrap(),
                        control.weight(),
                    )
                    .unwrap()
                })
                .collect(),
            edge.knots().to_vec(),
        )
        .unwrap();
        vertices.push(
            BrepVertex::try_new(edge.evaluate(*edge.domain().start()).unwrap(), 0.0).unwrap(),
        );
        edges.push(BrepEdge::try_new([index, index], edge, 0.0).unwrap());
        loops.push(
            BrepLoop::try_new(
                if index == 0 {
                    BrepLoopType::Outer
                } else {
                    BrepLoopType::Inner
                },
                vec![
                    BrepTrim::try_new(
                        [index, index],
                        Some(index),
                        false,
                        trim,
                        if capped {
                            BrepTrimType::Mated
                        } else {
                            BrepTrimType::Boundary
                        },
                        SurfaceIso::NotIso,
                        [0.0, 0.0],
                    )
                    .unwrap(),
                ],
            )
            .unwrap(),
        );
    }
    let mut faces = vec![BrepFace::try_new(surface, capped, loops.clone()).unwrap()];
    if capped {
        assert_eq!(radii.len(), 1);
        let z = radii[0] * radii[0];
        let cap = NurbsSurface::try_bilinear([
            point(-1.0, -1.0, z),
            point(1.0, -1.0, z),
            point(1.0, 1.0, z),
            point(-1.0, 1.0, z),
        ])
        .unwrap()
        .try_reparameterized(-1.0..=1.0, -1.0..=1.0)
        .unwrap();
        faces.push(BrepFace::try_new(cap, false, loops).unwrap());
    }
    Brep::try_new(vertices, edges, faces, Tolerance::DEFAULT).unwrap()
}

fn disk_area(radius: Real) -> Real {
    std::f64::consts::PI / 6.0 * ((1.0 + 4.0 * radius * radius).powf(1.5) - 1.0)
}

#[test]
fn integrates_nonplanar_rational_round_trims_and_holes() {
    for radii in [&[0.8][..], &[0.8, 0.35][..], &[0.8, 0.799][..]] {
        let brep = round_trim(paraboloid(), radii, false);
        let expected = disk_area(radii[0]) - radii.get(1).map_or(0.0, |radius| disk_area(*radius));
        assert!((brep.area(Tolerance::DEFAULT).unwrap() - expected).abs() < 1e-9);
    }
}

#[test]
fn integrates_trimmed_solid_volume_with_orientation_and_translation() {
    let brep = round_trim(paraboloid(), &[0.8], true);
    assert!(brep.is_solid());
    let expected = std::f64::consts::PI * 0.8_f64.powi(4) / 2.0;
    assert!((brep.signed_volume(Tolerance::DEFAULT).unwrap() - expected).abs() < 1e-9);
    assert!(
        (brep.area(Tolerance::DEFAULT).unwrap() - disk_area(0.8) - std::f64::consts::PI * 0.64)
            .abs()
            < 1e-9
    );
    let reversed = Brep::try_new(
        brep.vertices.clone(),
        brep.edges.clone(),
        brep.faces
            .iter()
            .map(|face| {
                BrepFace::try_new(face.surface.clone(), !face.reversed, face.loops.clone()).unwrap()
            })
            .collect(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    assert!((reversed.signed_volume(Tolerance::DEFAULT).unwrap() + expected).abs() < 1e-9);
    let translated = round_trim(paraboloid(), &[0.5], true)
        .transformed(
            crate::AffineTransform3::from_translation(Vector3::try_new(1e12, -2e12, 3e12).unwrap()),
            Tolerance::DEFAULT,
        )
        .unwrap();
    assert!(
        (translated.signed_volume(Tolerance::DEFAULT).unwrap() - std::f64::consts::PI / 32.0).abs()
            < 1e-10
    );
    assert!(
        (translated.area(Tolerance::DEFAULT).unwrap()
            - disk_area(0.5)
            - std::f64::consts::PI * 0.25)
            .abs()
            < 1e-10
    );
    assert!(
        (reversed.area(Tolerance::DEFAULT).unwrap() - brep.area(Tolerance::DEFAULT).unwrap()).abs()
            < 1e-9
    );
}

#[test]
fn integration_partitions_trim_crossings_at_surface_knots() {
    let refined = paraboloid()
        .try_insert_knot_u(-0.1, 2)
        .unwrap()
        .try_insert_knot_v(0.17, 2)
        .unwrap();
    let brep = round_trim(refined, &[0.8, 0.35], false);
    assert!(
        (brep.area(Tolerance::DEFAULT).unwrap() - disk_area(0.8) + disk_area(0.35)).abs() < 1e-9
    );
}

#[test]
fn mass_properties_are_invariant_under_uv_domain_scaling() {
    let original = round_trim(paraboloid(), &[0.5], true);
    let convert = |coordinate: Real, axis| {
        if axis == 0 {
            coordinate / 128.0 + 32.0
        } else {
            coordinate * 256.0 - 4096.0
        }
    };
    let faces = original
        .faces
        .iter()
        .map(|face| {
            let surface = face
                .surface
                .try_reparameterized(
                    convert(-1.0, 0)..=convert(1.0, 0),
                    convert(-1.0, 1)..=convert(1.0, 1),
                )
                .unwrap();
            let loops = face
                .loops
                .iter()
                .map(|face_loop| {
                    let trims = face_loop
                        .trims
                        .iter()
                        .map(|trim| {
                            let curve = NurbsCurve2::try_new_rational(
                                trim.curve.degree(),
                                trim.curve
                                    .control_points()
                                    .iter()
                                    .map(|control| {
                                        WeightedPoint2::try_new(
                                            Point2::try_new(
                                                convert(control.point().x(), 0),
                                                convert(control.point().y(), 1),
                                            )
                                            .unwrap(),
                                            control.weight(),
                                        )
                                        .unwrap()
                                    })
                                    .collect(),
                                trim.curve.knots().to_vec(),
                            )
                            .unwrap();
                            BrepTrim::try_new(
                                trim.vertices,
                                trim.edge,
                                trim.reversed_3d,
                                curve,
                                trim.trim_type,
                                trim.iso,
                                trim.tolerance,
                            )
                            .unwrap()
                        })
                        .collect();
                    BrepLoop::try_new(face_loop.loop_type, trims).unwrap()
                })
                .collect();
            BrepFace::try_new(surface, face.reversed, loops).unwrap()
        })
        .collect();
    let rescaled =
        Brep::try_new(original.vertices, original.edges, faces, Tolerance::DEFAULT).unwrap();
    assert!(
        (rescaled.area(Tolerance::DEFAULT).unwrap() - disk_area(0.5) - std::f64::consts::PI * 0.25)
            .abs()
            < 1e-9
    );
    assert!(
        (rescaled.signed_volume(Tolerance::DEFAULT).unwrap() - std::f64::consts::PI / 32.0).abs()
            < 1e-9
    );
}

fn triangular_trim(surface: NurbsSurface, diagonal: crate::NurbsCurve) -> Brep {
    let parameters = [
        Point2::try_new(0.0, 0.0).unwrap(),
        Point2::try_new(1.0, 0.0).unwrap(),
        Point2::try_new(1.0, 1.0).unwrap(),
    ];
    let vertices = parameters
        .iter()
        .map(|uv| BrepVertex::try_new(surface.evaluate(uv.x(), uv.y()).unwrap(), 0.0).unwrap())
        .collect();
    let curves = [
        surface.isocurve_u(0.0).unwrap(),
        surface.isocurve_v(1.0).unwrap(),
        diagonal.reversed().unwrap(),
    ];
    let edges = curves
        .into_iter()
        .enumerate()
        .map(|(index, curve)| BrepEdge::try_new([index, (index + 1) % 3], curve, 0.0).unwrap())
        .collect();
    let trims = (0..3)
        .map(|index| {
            BrepTrim::try_new(
                [index, (index + 1) % 3],
                Some(index),
                false,
                NurbsCurve2::try_line(parameters[index], parameters[(index + 1) % 3]).unwrap(),
                BrepTrimType::Boundary,
                [SurfaceIso::South, SurfaceIso::East, SurfaceIso::NotIso][index],
                [0.0, 0.0],
            )
            .unwrap()
        })
        .collect();
    Brep::try_new(
        vertices,
        edges,
        vec![
            BrepFace::try_new(
                surface,
                false,
                vec![BrepLoop::try_new(BrepLoopType::Outer, trims).unwrap()],
            )
            .unwrap(),
        ],
        Tolerance::DEFAULT,
    )
    .unwrap()
}

#[test]
fn resolves_narrow_surface_spans_in_both_parameter_directions() {
    let x = [0.0, 0.5, 0.5000001, 0.5000002, 1.0];
    let z = [0.0, 0.0, 0.2, 0.0, 0.0];
    let knots = vec![0.0, 0.0, 0.5, 0.5000001, 0.5000002, 1.0, 1.0];
    let surface = NurbsSurface::try_new(
        1,
        1,
        5,
        2,
        (0..2)
            .flat_map(|v| (0..5).map(move |u| point(x[u], v as Real, z[u])))
            .collect(),
        knots.clone(),
        vec![0.0, 0.0, 1.0, 1.0],
    )
    .unwrap();
    let diagonal =
        crate::NurbsCurve::try_new(1, (0..5).map(|i| point(x[i], x[i], z[i])).collect(), knots)
            .unwrap();
    for transpose in [false, true] {
        let expected = (0..4)
            .map(|i| {
                let midpoint = (x[i] + x[i + 1]) * 0.5;
                let width = if transpose { 1.0 - midpoint } else { midpoint };
                width * (x[i + 1] - x[i]).hypot(z[i + 1] - z[i])
            })
            .sum::<Real>();
        let source = if transpose {
            surface.try_swapped_uv().unwrap()
        } else {
            surface.clone()
        };
        let face = triangular_trim(source, diagonal.clone());
        assert!((face.area(Tolerance::DEFAULT).unwrap() - expected).abs() < 1e-9);
    }
}

#[test]
fn integrates_rational_cylinder_with_diagonal_uv_trim() {
    let radius = 2.0;
    let height = 3.0;
    let xy = [[radius, 0.0], [radius, radius], [0.0, radius]];
    let weights = [1.0, std::f64::consts::FRAC_1_SQRT_2, 1.0];
    let surface = NurbsSurface::try_new_rational(
        2,
        1,
        3,
        2,
        (0..2)
            .flat_map(|v| {
                (0..3).map(move |u| {
                    crate::WeightedPoint3::try_new(
                        point(xy[u][0], xy[u][1], height * v as Real),
                        weights[u],
                    )
                    .unwrap()
                })
            })
            .collect(),
        vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
        vec![0.0, 0.0, 1.0, 1.0],
    )
    .unwrap();
    let controls = (0..4)
        .map(|i| {
            let alpha = i as Real / 3.0;
            let left = if i > 0 { alpha * weights[i - 1] } else { 0.0 };
            let right = if i < 3 {
                (1.0 - alpha) * weights[i]
            } else {
                0.0
            };
            let weight = left + right;
            let coordinate = |axis| {
                (if i > 0 { left * xy[i - 1][axis] } else { 0.0 })
                    + (if i < 3 { right * xy[i][axis] } else { 0.0 })
            };
            crate::WeightedPoint3::try_new(
                point(
                    coordinate(0) / weight,
                    coordinate(1) / weight,
                    height * left / weight,
                ),
                weight,
            )
            .unwrap()
        })
        .collect();
    let diagonal = crate::NurbsCurve::try_new_rational(
        3,
        controls,
        vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0],
    )
    .unwrap();
    let brep = triangular_trim(surface, diagonal);
    assert!(
        (brep.area(Tolerance::DEFAULT).unwrap() - std::f64::consts::PI * radius * height / 4.0)
            .abs()
            < 1e-9
    );
}
