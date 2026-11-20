use super::*;
use crate::chain_selection::Session;
use viboceros_geometry::{ChainContinuity, ChainDirection, CurveChainOptions};

fn point(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}

fn line(document: &mut Document, a: Point3, b: Point3) -> ObjectId {
    document
        .add_geometry(Geometry::Line(
            LineSegment::try_new(a, b, document.tolerance()).unwrap(),
        ))
        .unwrap()
}

#[test]
fn sweep_two_rail_chains_match_collinear_and_kinked_rhino_fixtures() {
    for (bend, face_count, area) in [(0., 1, 10.000000000000002), (1., 2, 10.770329614269006)] {
        let mut doc = Document::default();
        let tolerance = doc.tolerance();
        let pairs = [0., 2.].map(|x| {
            // Each second segment is authored backward.
            let a = line(&mut doc, point(x, 0., 0.), point(x, bend, 2.5));
            let b = line(&mut doc, point(x, 0., 5.), point(x, bend, 2.5));
            [Target::Curve(a), Target::Curve(b)]
        });
        let sections = [0., 5.].map(|z| line(&mut doc, point(0., 0., z), point(2., 0., z)));
        let before = doc.objects().cloned().collect::<Vec<_>>();
        let rails = pairs.map(|[first, second]| {
            let mut picker = Session::new_for_rail(
                &doc,
                CurveChainOptions {
                    continuity: ChainContinuity::Position,
                    direction: ChainDirection::Both,
                    gap_tolerance: tolerance.absolute(),
                    angle_tolerance: tolerance.angular(),
                },
                &[],
            )
            .unwrap();
            picker.pick_input_with_growth(&doc, second, false).unwrap();
            picker.pick_input_with_growth(&doc, first, false).unwrap();
            assert!(picker.enter(&doc).unwrap());
            let path = picker.ordered_targets();
            assert_eq!(path, [(second, false), (first, true)]);
            Rail::chain(&doc, &path, tolerance.absolute()).unwrap()
        });
        let source = Source::with_rails(&doc, rails, &sections).unwrap();
        let brep = source.build(ShapeOptions::default(), tolerance).unwrap();
        let refitted = source
            .build(
                ShapeOptions {
                    refit_rails: true,
                    ..Default::default()
                },
                tolerance,
            )
            .unwrap();
        assert_eq!(brep.faces().len(), face_count);
        assert_eq!(refitted.faces().len(), face_count);
        assert_eq!(brep.edges().len(), if bend == 0. { 4 } else { 7 });
        assert!((brep.area(tolerance).unwrap() - area).abs() < 1e-8);
        assert!((refitted.area(tolerance).unwrap() - area).abs() < 1e-8);
        // Rhino's two captured outputs are rectangles on their source
        // planes. Validate the result across both parameter directions.
        for face in brep.faces() {
            let surface = face.surface();
            for u in 0..=12 {
                let u_domain = surface.domain_u();
                let u = u_domain.start() + (u_domain.end() - u_domain.start()) * f64::from(u) / 12.;
                for v in 0..=12 {
                    let v_domain = surface.domain_v();
                    let v =
                        v_domain.start() + (v_domain.end() - v_domain.start()) * f64::from(v) / 12.;
                    let p = surface.evaluate(u, v).unwrap();
                    let y = bend * (1. - (p.z() / 2.5 - 1.).abs());
                    assert!((p.y() - y).abs() < 1e-9, "{p:?}, expected y={y}");
                    assert!((-1e-9..=2. + 1e-9).contains(&p.x()));
                }
            }
        }
        assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
        assert_eq!(doc.selected_object_count(), 0);
    }
}
