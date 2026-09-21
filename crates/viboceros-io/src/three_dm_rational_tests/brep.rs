use super::*;
use viboceros_geometry::{
    Brep, BrepEdge, BrepFace, BrepLoop, BrepTrim, NurbsCurve2, WeightedPoint2,
};

fn scaled_controls(controls: &[WeightedPoint3], scale: f64) -> Vec<WeightedPoint3> {
    controls
        .iter()
        .map(|c| WeightedPoint3::try_new(c.point(), c.weight() * scale).unwrap())
        .collect()
}

fn scaled_surface(surface: &NurbsSurface, scale: f64) -> NurbsSurface {
    NurbsSurface::try_new_rational(
        surface.degree_u(),
        surface.degree_v(),
        surface.control_point_count_u(),
        surface.control_point_count_v(),
        scaled_controls(surface.control_points(), scale),
        surface.knots_u().to_vec(),
        surface.knots_v().to_vec(),
    )
    .unwrap()
}

fn surface() -> NurbsSurface {
    NurbsSurface::try_new_rational(
        1,
        1,
        2,
        2,
        [
            p(1.0, 1.0, 0.0),
            p(3.0, 1.0, 0.0),
            p(1.0, 3.0, 0.0),
            p(3.0, 3.0, 1.0),
        ]
        .map(|point| WeightedPoint3::try_new(point, 1.0).unwrap())
        .to_vec(),
        vec![0.0, 0.0, 1.0, 1.0],
        vec![0.0, 0.0, 1.0, 1.0],
    )
    .unwrap()
}

#[test]
fn coalesced_rational_brep_edges_and_trims_round_trip_without_new_full_order_knots() {
    let frame = viboceros_geometry::Frame3::try_from_normal(
        p(0., 0., 0.),
        viboceros_geometry::Vector3::try_new(0., 0., 1.).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    let original = Brep::try_cylinder(frame, 2., 0., 5., Tolerance::DEFAULT).unwrap();
    for edge in 0..original.edges().len() {
        let curve = original.edges()[edge].curve();
        let parameters = [0.125, 0.375, 0.875].map(|t| curve.parameter_at(t).unwrap());
        let source = original
            .try_split_edges_at_parameters(&[(edge, parameters.to_vec())], Tolerance::DEFAULT)
            .unwrap()
            .try_merge_all_edges(1e-10, Tolerance::DEFAULT)
            .unwrap();
        let ThreeDmGeometry::Brep(decoded) = round_trip(ThreeDmGeometry::Brep(source.clone()))
        else {
            panic!("expected B-rep")
        };
        assert!(decoded.is_solid());
        assert_eq!(decoded.vertices(), source.vertices());
        assert_eq!(decoded.edges().len(), original.edges().len());
        for (a, b) in decoded.edges().iter().zip(source.edges()) {
            assert_eq!(a.vertices(), b.vertices());
            assert_eq!(a.tolerance(), b.tolerance());
            curves_near(a.curve(), b.curve());
        }
        assert!(
            (decoded.signed_volume(Tolerance::DEFAULT).unwrap()
                - source.signed_volume(Tolerance::DEFAULT).unwrap())
            .abs()
                < 1e-10
        );
    }
}

#[test]
fn partitioned_closed_faces_keep_solid_incidence_and_round_trip() {
    use viboceros_geometry::{BrepTrimType, Frame3, SurfaceKnotDirection, Vector3};
    let frame = Frame3::try_from_normal(
        p(0., 0., 0.),
        Vector3::try_new(0., 0., 1.).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    let solids = [
        Brep::try_cylinder(frame, 2., 0., 5., Tolerance::DEFAULT).unwrap(),
        Brep::try_cone(frame, 2., 3., Tolerance::DEFAULT).unwrap(),
        Brep::try_surface_face(
            NurbsSurface::try_sphere(frame, 2.).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap(),
    ];
    for solid in solids {
        for reversed in [false, true] {
            let original = if reversed {
                solid.reversed()
            } else {
                solid.clone()
            };
            let side = original
                .faces()
                .iter()
                .position(|f| {
                    f.loops()
                        .iter()
                        .flat_map(|l| l.trims())
                        .any(|t| t.trim_type() == BrepTrimType::Seam)
                })
                .unwrap();
            let surface = original.faces()[side].surface();
            let cut = surface
                .knots_u()
                .iter()
                .copied()
                .find(|&k| k > *surface.domain_u().start())
                .unwrap();
            let source = original
                .try_split_face_at_knot(side, SurfaceKnotDirection::U, cut, Tolerance::DEFAULT)
                .unwrap()
                .unwrap();
            assert_eq!(source.faces().len(), original.faces().len() + 1);
            assert!(source.is_solid());
            assert!(source.edge_use_counts().iter().all(|&count| count == 2));
            for (i, face) in original
                .faces()
                .iter()
                .enumerate()
                .filter(|&(i, _)| i != side)
            {
                assert_eq!(source.faces()[i].surface(), face.surface());
                assert_eq!(
                    source.faces()[i].loops()[0].trims().len(),
                    face.loops()[0].trims().len() + 1
                );
            }
            let ThreeDmGeometry::Brep(decoded) = round_trip(ThreeDmGeometry::Brep(source.clone()))
            else {
                panic!("expected B-rep")
            };
            assert!(decoded.is_solid());
            assert_eq!(decoded.vertices(), source.vertices());
            assert_eq!(decoded.faces(), source.faces());
            assert_eq!(decoded.edges(), source.edges());
            assert!(
                (decoded.area(Tolerance::DEFAULT).unwrap()
                    - original.area(Tolerance::DEFAULT).unwrap())
                .abs()
                    < 1e-9
            );
            assert!(
                (decoded.signed_volume(Tolerance::DEFAULT).unwrap()
                    - original.signed_volume(Tolerance::DEFAULT).unwrap())
                .abs()
                    < 1e-9
            );
        }
    }
}

#[test]
fn brep_edges_uv_trims_and_signed_surfaces_share_safe_serialization() {
    let original = Brep::try_surface_face(surface(), Tolerance::DEFAULT).unwrap();
    for (edge_scale, trim_scale, surface_scale) in [
        (1e308, 1e-320, 1e-320),
        (1e-320, 1e308, -1.0),
        (1.0, -1e-320, -1e308),
    ] {
        let edges = original
            .edges()
            .iter()
            .map(|edge| {
                let c = edge.curve();
                BrepEdge::try_new(
                    edge.vertices(),
                    NurbsCurve::try_new_rational(
                        c.degree(),
                        scaled_controls(c.control_points(), edge_scale),
                        c.knots().to_vec(),
                    )
                    .unwrap(),
                    edge.tolerance(),
                )
                .unwrap()
            })
            .collect();
        let faces = original
            .faces()
            .iter()
            .map(|face| {
                let loops = face
                    .loops()
                    .iter()
                    .map(|boundary| {
                        let trims = boundary
                            .trims()
                            .iter()
                            .map(|trim| {
                                let c = trim.curve();
                                let controls = c
                                    .control_points()
                                    .iter()
                                    .map(|control| {
                                        WeightedPoint2::try_new(
                                            control.point(),
                                            control.weight() * trim_scale,
                                        )
                                        .unwrap()
                                    })
                                    .collect();
                                BrepTrim::try_new(
                                    trim.vertices(),
                                    trim.edge(),
                                    trim.is_reversed_3d(),
                                    NurbsCurve2::try_new_rational(
                                        c.degree(),
                                        controls,
                                        c.knots().to_vec(),
                                    )
                                    .unwrap(),
                                    trim.trim_type(),
                                    trim.iso(),
                                    trim.tolerance(),
                                )
                                .unwrap()
                            })
                            .collect();
                        BrepLoop::try_new(boundary.loop_type(), trims).unwrap()
                    })
                    .collect();
                BrepFace::try_new(
                    scaled_surface(face.surface(), surface_scale),
                    face.is_reversed(),
                    loops,
                )
                .unwrap()
            })
            .collect();
        let source = Brep::try_new(
            original.vertices().to_vec(),
            edges,
            faces,
            Tolerance::DEFAULT,
        )
        .unwrap();
        let ThreeDmGeometry::Brep(decoded) = round_trip(ThreeDmGeometry::Brep(source.clone()))
        else {
            panic!("expected B-rep")
        };
        assert_eq!(decoded.vertices(), source.vertices());
        assert_eq!(decoded.edges().len(), source.edges().len());
        assert_eq!(decoded.faces().len(), source.faces().len());
        for (actual, expected) in decoded.edges().iter().zip(source.edges()) {
            assert_eq!(actual.vertices(), expected.vertices());
            assert_eq!(actual.tolerance(), expected.tolerance());
            curves_near(actual.curve(), expected.curve());
        }
        for (actual, expected) in decoded.faces().iter().zip(source.faces()) {
            assert_eq!(actual.is_reversed(), expected.is_reversed());
            controls_near(
                actual.surface().control_points(),
                expected.surface().control_points(),
            );
            assert_eq!(actual.loops().len(), expected.loops().len());
            for (a, e) in actual.loops().iter().zip(expected.loops()) {
                assert_eq!(a.loop_type(), e.loop_type());
                assert_eq!(a.trims().len(), e.trims().len());
                for (a, e) in a.trims().iter().zip(e.trims()) {
                    assert_eq!(a.vertices(), e.vertices());
                    assert_eq!(a.edge(), e.edge());
                    assert_eq!(a.trim_type(), e.trim_type());
                    assert_eq!(a.iso(), e.iso());
                    assert_eq!(a.is_reversed_3d(), e.is_reversed_3d());
                    assert_eq!(a.tolerance(), e.tolerance());
                    assert_eq!(a.curve().knots(), e.curve().knots());
                    for (a, e) in a
                        .curve()
                        .control_points()
                        .iter()
                        .zip(e.curve().control_points())
                    {
                        assert_eq!(a.point(), e.point());
                    }
                }
            }
        }
    }
}
