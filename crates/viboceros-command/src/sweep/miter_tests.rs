use super::*;
use viboceros_geometry::{Circle3, Vector3};

fn point(x: Real, y: Real, z: Real) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}

#[test]
fn sweep_miters_place_a_closed_profile_around_the_rail_corner() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let tolerance = document.tolerance();
    registry
        .execute(&mut document, "Polyline 0,0,0 5,0,0 5,5,0")
        .unwrap();
    let mut ids = vec![document.objects().last().unwrap().id()];
    for (center, a, b, radius) in [
        (point(0., 0., 0.), 1., 0., 1.),
        (point(5., 0., 0.), 1., 1., 1.2),
        (point(5., 5., 0.), 0., 1., 1.5),
    ] {
        let unit = |x, y| {
            Vector3::try_new(x, y, 0.)
                .unwrap()
                .normalized_nonzero()
                .unwrap()
        };
        let circle =
            Circle3::try_from_frame(center, radius, unit(-b, a), unit(a, b), tolerance).unwrap();
        ids.push(document.add_geometry(Geometry::Circle(circle)).unwrap());
    }
    document
        .select_objects_direct(ids, SelectionMode::Replace)
        .unwrap();
    for command in ["Sweep1", "Sweep1 UntrimmedMiters=Yes RefitRail=Yes"] {
        registry.execute(&mut document, command).unwrap();
        let Geometry::Brep(brep) = document.objects().last().unwrap().geometry() else {
            panic!("expected a Sweep1 BRep");
        };
        // Rhino 8.32 with automatic profile placement and the natural seams.
        assert_eq!(
            (
                brep.faces().len(),
                brep.edges().len(),
                brep.vertices().len()
            ),
            (2, 5, 3)
        );
        assert!((brep.area(tolerance).unwrap() - 71.893_014_28).abs() < 2e-4);
        let first = brep.faces()[0].surface();
        let middle = first.isocurve_v(*first.domain_u().end()).unwrap();
        let q = 1.2 * std::f64::consts::FRAC_1_SQRT_2;
        let seam = middle.evaluate(*middle.domain().start()).unwrap();
        assert!(seam.distance_to(point(5. - q, q, 0.)).unwrap() < 1e-8);
        document.undo().unwrap();
    }
}

#[test]
fn sweep_miters_join_point_ends_to_a_shared_corner_profile() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    for command in ["Polyline 0,0,0 5,0,0 5,5,0", "Line 5,0,0 4,1,1"] {
        registry.execute(&mut document, command).unwrap();
    }
    let ids = document.objects().map(|o| o.id()).collect::<Vec<_>>();
    document
        .select_objects_direct(ids, SelectionMode::Replace)
        .unwrap();
    registry
        .execute(
            &mut document,
            "Sweep1 StartPoint=0,0,0 EndPoint=5,5,0 UntrimmedMiters=Yes",
        )
        .unwrap();
    let Geometry::Brep(brep) = document.objects().last().unwrap().geometry() else {
        panic!("expected a Sweep1 BRep");
    };
    assert_eq!(
        (
            brep.faces().len(),
            brep.edges().len(),
            brep.vertices().len()
        ),
        (2, 5, 4)
    );
    for (face, pole, u) in [
        (&brep.faces()[0], point(0., 0., 0.), 0.),
        (&brep.faces()[1], point(5., 5., 0.), 10.),
    ] {
        let c = face.surface().isocurve_v(u).unwrap();
        assert!(c.control_points().iter().all(|p| p.point() == pole));
    }
    assert!((brep.area(document.tolerance()).unwrap() - 5. * 2_f64.sqrt()).abs() < 1e-7);
}
