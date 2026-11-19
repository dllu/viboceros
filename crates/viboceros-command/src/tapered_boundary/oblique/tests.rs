use super::*;
use viboceros_geometry::{Circle3, UnitVector3};

fn point(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}

fn circle(tolerance: Tolerance) -> NurbsCurve {
    Circle3::try_new(
        point(0., 0., 0.),
        3.,
        UnitVector3::try_new(0., 0., 1., tolerance).unwrap(),
        tolerance,
    )
    .unwrap()
    .to_nurbs()
    .unwrap()
}

#[test]
fn oblique_boundary_matches_rhinos_closed_continuous_rims() {
    let tolerance = Tolerance::try_new(1e-6, 1e-12, 1e-8).unwrap();
    let circle = circle(tolerance);
    for (surface, cap, height, slope, width, draft, capture_volume, capture_max) in [
        (
            false,
            true,
            5.,
            0.,
            10.,
            -10.,
            Some(188.04637076061877),
            [4.766893399950148, 3.893886442131147, 5.],
        ),
        (
            false,
            false,
            5.,
            0.,
            24.,
            -10.,
            None,
            [4.7668933999501455, 3.8938864442720913, 5.],
        ),
        (
            false,
            true,
            -5.,
            0.,
            24.,
            -10.,
            Some(188.04637076061852),
            [3.046922616331543, 3.893886442131147, 0.],
        ),
        (
            true,
            false,
            5.,
            0.2,
            24.,
            -10.,
            Some(197.37360108701105),
            [5.129418852599681, 3.92844604176103, 6.025883770519936],
        ),
        (
            true,
            true,
            5.,
            0.,
            10.,
            10.,
            Some(103.18910290878084),
            [3., 3., 5.],
        ),
        (
            false,
            true,
            5.,
            0.2,
            24.,
            2.,
            Some(137.3940706255841),
            [3.7532403335424696, 3., 5.750648066708493],
        ),
    ] {
        let mut doc = Document::new(tolerance);
        let registry = CommandRegistry::with_builtins();
        let (command, input) = if surface {
            (
                "ExtrudeSrfTapered",
                Geometry::Brep(Brep::try_planar_face(&circle, tolerance).unwrap()),
            )
        } else {
            ("ExtrudeCrvTapered", Geometry::NurbsCurve(circle.clone()))
        };
        let source = doc.add_geometry(input).unwrap();
        let half = width / 2.;
        let (left, right) = (1. - half, 1. + half);
        let corner = |x, y| point(x, y, height + slope * x);
        // Reverse the cutter's UV orientation to exercise cap reorientation.
        let support = NurbsSurface::try_bilinear([
            corner(left, -half),
            corner(left, half),
            corner(right, half),
            corner(right, -half),
        ])
        .unwrap();
        let cutter = doc
            .add_geometry(Geometry::NurbsSurface(support.clone()))
            .unwrap();
        doc.select_objects_direct([source], SelectionMode::Replace)
            .unwrap();
        doc.clear_history().unwrap();
        let request = format!(
            "{command} ToBoundary={cutter}:0 Direction=1,0,6 DraftAngle={draft} Solid={}",
            if cap { "Yes" } else { "No" }
        );
        registry
            .execute(&mut doc, &request)
            .unwrap_or_else(|error| panic!("{request} {height} {slope}: {error}"));
        let result = doc
            .objects()
            .find(|obj| ![source, cutter].contains(&obj.id()))
            .unwrap();
        let Geometry::Brep(brep) = result.geometry() else {
            panic!("expected a cut B-rep")
        };
        assert_eq!(brep.edges().len(), 12);
        assert_eq!(
            brep.faces().len(),
            if capture_volume.is_some() { 6 } else { 4 }
        );
        assert_eq!(brep.is_solid(), capture_volume.is_some());
        if let Some(expected) = capture_volume {
            let volume = brep.signed_volume(tolerance).unwrap();
            assert!((volume - expected).abs() < 1e-4, "{volume} vs {expected}");
            assert!(brep.faces().iter().any(|face| face.surface() == &support));
        }
        let bounds = Shape::from_brep(brep, tolerance)
            .unwrap()
            .tight_bounds()
            .unwrap();
        let expected_max = Point3::try_from(capture_max).unwrap();
        assert!(
            bounds.max().distance_to(expected_max).unwrap() < tolerance.absolute(),
            "{bounds:?} vs {expected_max:?}"
        );
        assert_eq!(doc.selected_object_count(), 2);
        registry.execute(&mut doc, "Undo").unwrap();
        assert_eq!(doc.objects().len(), 2);
        registry.execute(&mut doc, "Redo").unwrap();
        assert_eq!(doc.objects().len(), 3);
    }
}

#[test]
fn oblique_boundary_keeps_the_selected_warped_face_as_its_cap() {
    let tolerance = Tolerance::try_new(1e-6, 1e-12, 1e-8).unwrap();
    let circle = circle(tolerance);
    let chart = NurbsSurface::try_bilinear([
        point(-9., -9., 4.3),
        point(15., -9., 5.5),
        point(15., 13., 6.8),
        point(-9., 13., 5.2),
    ])
    .unwrap();
    assert!(chart.plane(tolerance).unwrap().is_none());
    let positive_bounds = [
        point(-3.0469832307168483, -3.90123097701401, 0.),
        point(4.929981805866948, 3.9700790037126144, 5.538213296071267),
    ];
    let reverse = AffineTransform3::try_nonuniform_scale(point(0., 0., 0.), [1., 1., -1.]).unwrap();
    for (command, source, chart, solid, volume, expected) in [
        (
            "ExtrudeCrvTapered",
            Geometry::NurbsCurve(circle.clone()),
            chart.clone(),
            "Yes",
            Some(199.2882349453992),
            positive_bounds,
        ),
        (
            "ExtrudeSrfTapered",
            Geometry::Brep(Brep::try_planar_face(&circle, tolerance).unwrap()),
            chart.try_swapped_uv().unwrap(),
            "No",
            Some(199.28823221440032),
            positive_bounds,
        ),
        (
            "ExtrudeCrvTapered",
            Geometry::NurbsCurve(circle.clone()),
            chart.clone(),
            "No",
            None,
            positive_bounds,
        ),
        (
            "ExtrudeCrvTapered",
            Geometry::NurbsCurve(circle),
            chart.transformed(reverse).unwrap(),
            "Yes",
            Some(194.3590570699962),
            [
                point(-4.735725588781621, -3.8848178324519647, -5.4258245466862505),
                point(3.05023441968352, 3.950145424158397, 0.),
            ],
        ),
    ] {
        let mut doc = Document::new(tolerance);
        let registry = CommandRegistry::with_builtins();
        let source = doc.add_geometry(source).unwrap();
        let cutter = doc
            .add_geometry(Geometry::NurbsSurface(chart.clone()))
            .unwrap();
        doc.select_objects_direct([source], SelectionMode::Replace)
            .unwrap();
        registry
            .execute(
                &mut doc,
                &format!(
                    "{command} ToBoundary={cutter} Direction=1,0,6 DraftAngle=-10 Solid={solid}"
                ),
            )
            .unwrap();
        let Geometry::Brep(brep) = doc
            .objects()
            .find(|obj| ![source, cutter].contains(&obj.id()))
            .unwrap()
            .geometry()
        else {
            panic!("expected an oblique cut")
        };
        assert_eq!(brep.edges().len(), 12);
        assert_eq!(brep.faces().len(), if volume.is_some() { 6 } else { 4 });
        assert_eq!(brep.is_solid(), volume.is_some());
        if let Some(volume) = volume {
            assert!(brep.faces().iter().any(|face| face.surface() == &chart));
            let mass = brep.signed_volume(tolerance).unwrap();
            assert!((mass - volume).abs() < 1e-4, "{mass} vs {volume}");
        }
        let bounds = Shape::from_brep(brep, tolerance)
            .unwrap()
            .tight_bounds()
            .unwrap();
        for (actual, reference) in [bounds.min(), bounds.max()].into_iter().zip(expected) {
            assert!(
                actual.distance_to(reference).unwrap() < tolerance.absolute(),
                "{actual:?} vs {reference:?}"
            );
        }
    }
}
