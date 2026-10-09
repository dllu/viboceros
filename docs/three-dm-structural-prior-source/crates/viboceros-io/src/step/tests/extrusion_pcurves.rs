use super::*;
use monstertruck::meshing::prelude::{ParametricCurve, ParametricSurface};
use monstertruck::modeling::{
    BsplineCurve, KnotVector, Line, NurbsCurve as TruckNurbsCurve, Point2 as P2, Point3 as P3,
    Vector3, Vector4,
};
use monstertruck::step::load::step_geometry::{
    Curve2D, Curve3D, StepExtrusionSurface, StepParameterCurve, Surface, SweepSurface,
};
use monstertruck::step::save::StepModels;
use monstertruck::topology::compress::{
    CompressedEdge, CompressedEdgeUse, CompressedTrimmedFace, CompressedTrimmedShell,
};

#[test]
fn serialized_extrusion_curved_pcurve_import_retains_native_surfaces_and_certified_edges() {
    for rational in [false, true] {
        let directrix = if rational {
            Curve3D::NurbsCurve(TruckNurbsCurve::new(BsplineCurve::new(
                KnotVector::bezier_knot(2),
                vec![
                    Vector4::new(0., 0., 0., 1.),
                    Vector4::new(0.5, 0., 0.5, 0.5),
                    Vector4::new(2., 0., 0., 1.),
                ],
            )))
        } else {
            Curve3D::BsplineCurve(BsplineCurve::new(
                KnotVector::bezier_knot(2),
                vec![
                    P3::new(0., 0., 0.),
                    P3::new(1., 0., 1.),
                    P3::new(2., 0., 0.),
                ],
            ))
        };
        let surface = Surface::SweepSurface(SweepSurface::ExtrusionSurface(
            StepExtrusionSurface::by_extrusion(directrix, Vector3::new(0., 3., 0.)),
        ));
        let points = [P2::new(0., 0.), P2::new(1., 0.), P2::new(1., 1.)];
        let curved = Curve2D::NurbsCurve(TruckNurbsCurve::new(BsplineCurve::new(
            KnotVector::from(vec![5., 5., 5., 9., 9., 9.]),
            vec![
                Vector3::new(0., 0., 1.),
                Vector3::new(0.15, 0.6, 0.75),
                Vector3::new(1.5, 1.5, 1.5),
            ],
        )));
        let curves = [
            Curve2D::Line(Line(points[0], points[1])),
            Curve2D::Line(Line(points[1], points[2])),
            curved.clone(),
        ];
        let parameter = |curve| StepParameterCurve::new(Box::new(curve), Box::new(surface.clone()));
        let shell = CompressedTrimmedShell {
            vertices: points
                .into_iter()
                .map(|p| surface.evaluate(p.x, p.y))
                .collect(),
            edges: curves
                .iter()
                .cloned()
                .enumerate()
                .map(|(i, c)| CompressedEdge {
                    vertices: if i == 2 { (0, 2) } else { (i, i + 1) },
                    curve: Curve3D::ParameterCurve(parameter(c)),
                })
                .collect(),
            faces: vec![CompressedTrimmedFace {
                boundaries: vec![
                    curves
                        .into_iter()
                        .enumerate()
                        .map(|(index, c)| CompressedEdgeUse {
                            index,
                            orientation: index < 2,
                            trim_curve: Some(parameter(c)),
                        })
                        .collect(),
                ],
                orientation: true,
                surface: surface.clone(),
            }],
        };
        let mut models = StepModels::default();
        models.push_trimmed_shell(&shell);
        let text = CompleteStepDisplay::new(models, StepHeaderDescriptor::default()).to_string();
        assert!(text.contains("SURFACE_OF_LINEAR_EXTRUSION("));
        assert!(text.contains("PCURVE("));
        let native = read_step_native_instances(Cursor::new(text), Tolerance::DEFAULT).unwrap();
        assert_eq!(native.instances.len(), 1);
        let brep = &native.instances[0].brep;
        assert_eq!(brep.faces().len(), 1);
        assert_eq!(brep.edges().len(), 3);
        assert_eq!(brep.faces()[0].surface().degree_u(), 2);
        assert_eq!(brep.faces()[0].surface().degree_v(), 1);
        assert_eq!(brep.edges()[2].curve().domain(), 5.0..=9.0);
        for trim in brep.faces()[0].loops()[0].trims() {
            let spatial = brep.edges()[trim.edge().unwrap()].curve();
            let oriented = if trim.is_reversed_3d() {
                spatial.reversed().unwrap()
            } else {
                spatial.clone()
            };
            assert!(
                brep.faces()[0]
                    .surface()
                    .parameter_curve_deviation_bound(
                        trim.curve(),
                        &oriented,
                        Tolerance::DEFAULT.absolute()
                    )
                    .unwrap()
                    .is_some()
            );
        }
        for i in 0..=32 {
            let t = 5. + 4. * i as f64 / 32.;
            let uv = curved.evaluate(t);
            let expected = surface.evaluate(uv.x, uv.y);
            let actual = brep.edges()[2].curve().evaluate(t).unwrap();
            assert!((actual.x() - expected.x).abs() < 1e-6);
            assert!((actual.y() - expected.y).abs() < 1e-6);
            assert!((actual.z() - expected.z).abs() < 1e-6);
        }
    }
}
