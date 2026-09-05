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
