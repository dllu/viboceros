use super::*;
use crate::{ParameterSide, TessellationNormalSource};

fn p(x: Real, y: Real, z: Real) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn uv(x: Real, y: Real) -> Point2 {
    Point2::try_new(x, y).unwrap()
}
fn fold(height: Real) -> NurbsSurface {
    NurbsSurface::try_new(
        1,
        1,
        3,
        2,
        vec![
            p(0., 0., 0.),
            p(1., 0., 0.),
            p(2., 0., height),
            p(0., 1., 0.),
            p(1., 1., 0.),
            p(2., 1., height),
        ],
        vec![0., 0., 1., 2., 2.],
        vec![0., 0., 1., 1.],
    )
    .unwrap()
}
fn trimmed(
    surface: NurbsSurface,
    parameters: &[Point2],
    curve_breaks: bool,
    tolerance: Tolerance,
) -> Brep {
    let points = parameters
        .iter()
        .map(|q| surface.evaluate(q.x(), q.y()).unwrap())
        .collect::<Vec<_>>();
    let vertices = points
        .iter()
        .map(|&p| BrepVertex::try_new(p, 0.).unwrap())
        .collect();
    let mut edges = Vec::new();
    let mut trims = Vec::new();
    for i in 0..points.len() {
        let j = (i + 1) % points.len();
        let a = parameters[i];
        let b = parameters[j];
        let crosses = a.x().min(b.x()) < 1. && a.x().max(b.x()) > 1.;
        let (controls, knots) = if curve_breaks && crosses {
            let t = (1. - a.x()) / (b.x() - a.x());
            let v = crate::interpolate_scalar([a.y(), b.y()], [0., 1.], t).unwrap();
            (
                vec![points[i], surface.evaluate(1., v).unwrap(), points[j]],
                vec![0., 0., t, 1., 1.],
            )
        } else {
            (vec![points[i], points[j]], vec![0., 0., 1., 1.])
        };
        edges.push(
            BrepEdge::try_new([i, j], NurbsCurve::try_new(1, controls, knots).unwrap(), 0.)
                .unwrap(),
        );
        trims.push(
            BrepTrim::try_new(
                [i, j],
                Some(i),
                false,
                NurbsCurve2::try_line(a, b).unwrap(),
                BrepTrimType::Boundary,
                SurfaceIso::NotIso,
                [0.; 2],
            )
            .unwrap(),
        );
    }
    let face = BrepFace::try_new(
        surface,
        false,
        vec![BrepLoop::try_new(BrepLoopType::Outer, trims).unwrap()],
    )
    .unwrap();
    Brep::try_new(vertices, edges, vec![face], tolerance).unwrap()
}

fn triangle_positions(mesh: &TriangleMesh) -> BTreeSet<[[u64; 3]; 3]> {
    mesh.triangles()
        .iter()
        .map(|t| {
            let mut points = t.map(|i| mesh.vertices()[i as usize].to_array().map(Real::to_bits));
            points.sort();
            points
        })
        .collect()
}

#[test]
fn diamond_crease_topology_and_normals_are_independent_of_trim_loop_start() {
    let mut parameters = vec![uv(1.01, 0.34), uv(1., 0.58), uv(0.99, 0.34), uv(1., 0.1)];
    let mut expected = None;
    for _ in 0..4 {
        let source = trimmed(fold(1.), &parameters, false, Tolerance::DEFAULT);
        let old_mesh = source.tessellate(1, Tolerance::DEFAULT).unwrap();
        let output = source
            .tessellate_with_normals(1, Tolerance::DEFAULT)
            .unwrap();
        assert_eq!(output.mesh.triangles().len(), 2);
        assert_eq!(output.mesh.topology().boundary_edge_count(), 4);
        assert!(output.mesh.topology().is_manifold());
        assert!(output.mesh.topology().is_oriented());
        let positions = triangle_positions(&output.mesh);
        if let Some(expected) = &expected {
            assert_eq!(&positions, expected);
        } else {
            expected = Some(positions);
        }
        for samples in &output.corners {
            let left = samples.iter().all(|s| s.uv.x() <= 1.);
            let right = samples.iter().all(|s| s.uv.x() >= 1.);
            assert!(left || right);
            for sample in samples {
                let normal = sample.normal.unwrap();
                assert_eq!(sample.normal_source, TessellationNormalSource::Analytic);
                if left {
                    assert!(normal.x().abs() < 1e-12 && normal.z() > 1. - 1e-12);
                } else {
                    assert!(normal.x() < -0.7 && normal.z() > 0.7);
                }
            }
        }
        // Opt-in constraints must not change the plain API, even for a loop
        // order whose old coarse triangulation crossed the crease.
        assert_eq!(old_mesh, source.tessellate(1, Tolerance::DEFAULT).unwrap());
        parameters.rotate_left(1);
    }
}

#[test]
fn missing_independent_anchors_retry_the_shared_edge_sample_table() {
    let parameters = [uv(0.8, 0.1), uv(1.2, 0.3), uv(1.2, 0.7), uv(0.8, 0.9)];
    let source = trimmed(fold(1.), &parameters, true, Tolerance::DEFAULT);
    let output = source
        .tessellate_with_normals(1, Tolerance::DEFAULT)
        .unwrap();
    assert!(
        output
            .corners
            .iter()
            .flatten()
            .any(|c| c.uv.x() == 1. && (c.uv.y() - 0.2).abs() < 1e-12)
    );
    assert!(
        output
            .corners
            .iter()
            .flatten()
            .any(|c| c.uv.x() == 1. && (c.uv.y() - 0.8).abs() < 1e-12)
    );
    for samples in &output.corners {
        assert!(samples.iter().all(|s| s.uv.x() <= 1.) || samples.iter().all(|s| s.uv.x() >= 1.));
    }
    assert_eq!(output.mesh.topology().boundary_edge_count(), 6);
    assert!(output.mesh.topology().is_manifold() && output.mesh.topology().is_oriented());
}

#[test]
fn missing_shared_anchors_return_explicit_unsupported_instead_of_crossing_a_crease() {
    let parameters = [uv(0.8, 0.1), uv(1.2, 0.3), uv(1.2, 0.7), uv(0.8, 0.9)];
    let tolerance = Tolerance::try_new(1e-4, 1e-12, 1e-10).unwrap();
    // Portable B-reps allow an approximated edge within the caller's model
    // tolerance. Its smooth curve has no sample at the actual support crease.
    let source = trimmed(fold(1e-5), &parameters, false, tolerance);
    let plain = source.tessellate(1, tolerance).unwrap();
    let original = source.clone();
    assert!(matches!(
        source.tessellate_with_normals(1, tolerance),
        Err(GeometryError::UnsupportedBrepNormalCrease {
            face: 0,
            direction: "U",
            parameter: 1.
        })
    ));
    assert_eq!(source, original);
    assert_eq!(source.tessellate(1, tolerance).unwrap(), plain);
}

#[test]
fn translated_local_charts_keep_trimmed_crease_positions_and_normals() {
    let parameters = [
        uv(1.015625, 0.375),
        uv(1., 0.625),
        uv(0.984375, 0.375),
        uv(1., 0.125),
    ];
    let mut source = trimmed(fold(1.), &parameters, false, Tolerance::DEFAULT);
    source.faces[0] =
        crate::brep::parameter_frame::tests::translated_face(&source.faces[0], [1e12, -2e12]);
    source.validate(Tolerance::DEFAULT).unwrap();
    let output = source
        .tessellate_with_normals(3, Tolerance::DEFAULT)
        .unwrap();
    for (triangle, samples) in output.mesh.triangles().iter().zip(&output.corners) {
        let left = samples.iter().all(|s| s.uv.x() <= 1.);
        let right = samples.iter().all(|s| s.uv.x() >= 1.);
        assert!(left || right);
        for (&index, sample) in triangle.iter().zip(samples) {
            assert_eq!(sample.uv_origin[0], 1e12);
            let model_v = sample.uv.y() + (sample.uv_origin[1] - (-2e12));
            let expected = p(sample.uv.x(), model_v, (sample.uv.x() - 1.).max(0.));
            assert!(
                output.mesh.vertices()[index as usize]
                    .distance_to(expected)
                    .unwrap()
                    < 1e-12
            );
            let expected = fold(1.)
                .normal_at_on_sides(
                    sample.uv.x(),
                    model_v,
                    if left {
                        ParameterSide::Left
                    } else {
                        ParameterSide::Right
                    },
                    ParameterSide::Right,
                )
                .unwrap();
            assert!(
                expected
                    .as_vector()
                    .dot(sample.normal.unwrap().as_vector())
                    .unwrap()
                    > 1. - 1e-12
            );
        }
    }
}

#[test]
fn singular_corners_keep_sampled_limits_in_translated_charts() {
    let surface =
        NurbsSurface::try_bilinear([p(0., 0., 0.), p(1., 0., 0.), p(1., 1., 0.), p(0., 0., 0.)])
            .unwrap()
            .try_reparameterized(1e12..=1e12 + 1., -2e12..=-2e12 + 1.)
            .unwrap();
    let source = Brep::try_surface_face(surface, Tolerance::DEFAULT).unwrap();
    let output = source
        .tessellate_with_normals(3, Tolerance::DEFAULT)
        .unwrap();
    assert!(
        output
            .corners
            .iter()
            .flatten()
            .any(|c| c.normal_source == TessellationNormalSource::SampledLimit)
    );
    assert!(
        output
            .corners
            .iter()
            .flatten()
            .all(|c| c.normal.unwrap().z() > 1. - 1e-12)
    );
    assert_eq!(
        output.mesh,
        source.tessellate(3, Tolerance::DEFAULT).unwrap()
    );
}

#[test]
fn exact_c1_insertions_do_not_create_spurious_normal_constraints() {
    let surface =
        NurbsSurface::try_bilinear([p(0., 0., 0.), p(1., 0., 0.), p(1., 1., 0.), p(0., 1., 0.)])
            .unwrap()
            .try_insert_knot_u(0.23, 1)
            .unwrap()
            .try_insert_knot_v(0.71, 1)
            .unwrap();
    assert!(self::super::parameters(&surface).iter().all(Vec::is_empty));
}

#[test]
fn crease_constraints_on_a_trimmed_solid_do_not_create_seam_t_junctions() {
    let parameters = [uv(1.01, 0.34), uv(1., 0.58), uv(0.99, 0.34), uv(1., 0.1)];
    let surface = fold(1.);
    let points = parameters.map(|p| surface.evaluate(p.x(), p.y()).unwrap());
    let mut parts = vec![trimmed(surface, &parameters, false, Tolerance::DEFAULT)];
    let bottom = NurbsSurface::try_bilinear([
        p(0., 0., -1.),
        p(2., 0., -1.),
        p(2., 1., -1.),
        p(0., 1., -1.),
    ])
    .unwrap()
    .try_reparameterized(0.0..=2., 0.0..=1.)
    .unwrap();
    parts.push(trimmed(bottom, &parameters, false, Tolerance::DEFAULT));
    for i in 0..4 {
        let a = points[i];
        let b = points[(i + 1) % 4];
        let wall =
            NurbsSurface::try_bilinear([a, b, p(b.x(), b.y(), -1.), p(a.x(), a.y(), -1.)]).unwrap();
        parts.push(Brep::try_surface_face(wall, Tolerance::DEFAULT).unwrap());
    }
    let mut joined = crate::join_breps(
        &parts.iter().collect::<Vec<_>>(),
        Tolerance::DEFAULT.absolute(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    assert_eq!(joined.len(), 1);
    let source = joined.remove(0).brep;
    assert!(source.is_solid());
    let output = source
        .tessellate_with_normals(1, Tolerance::DEFAULT)
        .unwrap();
    let topology = output.mesh.topology();
    assert!(topology.is_closed());
    assert!(topology.is_manifold());
    assert!(topology.is_oriented());
    assert_eq!(topology.boundary_edge_count(), 0);
    for triangle in &output.corners {
        if triangle[0].face == Some(0) {
            assert!(
                triangle.iter().all(|c| c.uv.x() <= 1.) || triangle.iter().all(|c| c.uv.x() >= 1.)
            );
        }
    }
}

#[test]
fn intersecting_u_and_v_creases_share_one_interior_station() {
    let controls = (0..3)
        .flat_map(|v| {
            (0..3).map(move |u| {
                p(
                    u as Real,
                    v as Real,
                    usize::from(u == 2) as Real + usize::from(v == 2) as Real,
                )
            })
        })
        .collect();
    let surface = NurbsSurface::try_new(
        1,
        1,
        3,
        3,
        controls,
        vec![0., 0., 1., 2., 2.],
        vec![0., 0., 1., 2., 2.],
    )
    .unwrap();
    let parameters = [uv(1.01, 1.), uv(1., 1.3), uv(0.99, 1.), uv(1., 0.7)];
    let mut source = trimmed(surface, &parameters, false, Tolerance::DEFAULT);
    for reversed in [false, true] {
        source.faces[0].reversed = reversed;
        let output = source
            .tessellate_with_normals(1, Tolerance::DEFAULT)
            .unwrap();
        assert_eq!(output.mesh.triangles().len(), 4);
        assert_eq!(output.mesh.topology().boundary_edge_count(), 4);
        assert!(output.mesh.topology().is_oriented() && output.mesh.topology().is_manifold());
        let mut center_normals = BTreeSet::new();
        for samples in &output.corners {
            let left_u = samples.iter().all(|c| c.uv.x() <= 1.);
            let left_v = samples.iter().all(|c| c.uv.y() <= 1.);
            assert!(left_u || samples.iter().all(|c| c.uv.x() >= 1.));
            assert!(left_v || samples.iter().all(|c| c.uv.y() >= 1.));
            for sample in samples {
                let normal = source.faces[0]
                    .surface
                    .normal_at_on_sides(
                        sample.uv.x(),
                        sample.uv.y(),
                        if left_u {
                            ParameterSide::Left
                        } else {
                            ParameterSide::Right
                        },
                        if left_v {
                            ParameterSide::Left
                        } else {
                            ParameterSide::Right
                        },
                    )
                    .unwrap();
                let normal = if reversed { normal.opposite() } else { normal };
                assert!(
                    normal
                        .as_vector()
                        .dot(sample.normal.unwrap().as_vector())
                        .unwrap()
                        > 1. - 1e-12
                );
                if sample.uv == uv(1., 1.) {
                    center_normals.insert(
                        sample
                            .normal
                            .unwrap()
                            .as_vector()
                            .to_array()
                            .map(|c| (c * 1e6).round() as i64),
                    );
                }
            }
        }
        assert_eq!(center_normals.len(), 4);
    }
}
