use super::*;
use viboceros_geometry::{
    CircularArc3, CurveSegment3, LineSegment, NurbsCurve, PolyCurve3, UnitVector3,
};

fn p(x: Real, y: Real, z: Real) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}

#[test]
fn straight_round_pipe_matches_local_and_global_rhino_volumes() {
    let mut document = Document::default();
    let source = document
        .add_geometry(Geometry::Line(
            LineSegment::try_new(p(0., 0., 0.), p(0., 0., 5.), Tolerance::DEFAULT).unwrap(),
        ))
        .unwrap();
    let registry = CommandRegistry::with_builtins();
    let slope = 0.2_f64;
    let sphere_scale = slope.hypot(1.0);
    let spherical_cap_volume = |radius: Real, segment_height: Real| {
        std::f64::consts::PI
            * segment_height.powi(2)
            * (radius * sphere_scale - segment_height / 3.0)
    };
    let global = std::f64::consts::PI * 5.0 * 7.0 / 3.0
        + spherical_cap_volume(1.0, sphere_scale + slope)
        + spherical_cap_volume(2.0, 2.0 * (sphere_scale - slope));
    for (arguments, expected) in [
        (
            "1",
            std::f64::consts::PI * 5.0 + 4.0 * std::f64::consts::PI / 3.0,
        ),
        (
            "1 2",
            std::f64::consts::PI * 5.0 * (2.0 + 13.0 / 35.0)
                + 2.0 * std::f64::consts::PI * 9.0 / 3.0,
        ),
        (
            "2 1",
            std::f64::consts::PI * 5.0 * (2.0 + 13.0 / 35.0)
                + 2.0 * std::f64::consts::PI * 9.0 / 3.0,
        ),
        ("1 2 ShapeBlending=Global", global),
        ("2 1 ShapeBlending=Global", global),
    ] {
        registry
            .execute(
                &mut document,
                &format!("Pipe {source} {arguments} Cap=Round"),
            )
            .unwrap();
        let Geometry::Brep(pipe) = document.objects().last().unwrap().geometry() else {
            panic!("round pipe should be a B-rep")
        };
        assert!(pipe.is_closed());
        assert!(pipe.is_solid());
        assert_eq!(pipe.faces().len(), 3);
        let measured = pipe.signed_volume(Tolerance::DEFAULT).unwrap();
        assert!(
            (measured - expected).abs() / expected < 1e-8,
            "{measured} vs {expected}"
        );
    }
}

#[test]
fn straight_round_thick_pipe_matches_rhino_planar_caps() {
    let mut document = Document::default();
    let source = document
        .add_geometry(Geometry::Line(
            LineSegment::try_new(p(0., 0., 0.), p(0., 0., 5.), Tolerance::DEFAULT).unwrap(),
        ))
        .unwrap();
    CommandRegistry::with_builtins()
        .execute(
            &mut document,
            &format!("Pipe {source} 1 WallThickness=0.5 Cap=Round"),
        )
        .unwrap();
    let Geometry::Brep(pipe) = document.objects().last().unwrap().geometry() else {
        panic!("round thick pipe should be a B-rep")
    };
    assert!(pipe.is_closed());
    assert!(pipe.is_solid());
    assert_eq!(pipe.faces().len(), 4);
    let expected = std::f64::consts::PI * (1.5_f64.powi(2) - 1.0) * 5.0;
    let measured = pipe.signed_volume(Tolerance::DEFAULT).unwrap();
    assert!(
        (measured - expected).abs() / expected < 1e-8,
        "{measured} vs {expected}"
    );
}

#[test]
fn straight_pipe_uses_local_cubic_and_global_conical_radii() {
    let mut document = Document::default();
    let source = document
        .add_geometry(Geometry::Line(
            LineSegment::try_new(p(0., 0., 0.), p(0., 0., 5.), Tolerance::DEFAULT).unwrap(),
        ))
        .unwrap();
    let registry = CommandRegistry::with_builtins();
    registry
        .execute(&mut document, &format!("Pipe {source} 1"))
        .unwrap();
    let Geometry::Brep(cylinder) = document.objects().last().unwrap().geometry() else {
        panic!("flat pipe should be a B-rep")
    };
    assert!(cylinder.is_closed());
    assert_eq!(cylinder.faces().len(), 3);
    registry
        .execute(&mut document, &format!("Pipe {source} 1 2 Cap=None"))
        .unwrap();
    let Geometry::NurbsSurface(local) = document.objects().last().unwrap().geometry() else {
        panic!("open local pipe should be a NURBS surface")
    };
    assert_eq!(local.degree_v(), 3);
    let sample = local
        .evaluate(*local.domain_u().start(), 0.2 * *local.domain_v().end())
        .unwrap();
    assert!((sample.z() - 1.0).abs() < 1e-10);
    assert!((sample.x().hypot(sample.y()) - 1.104).abs() < 1e-10);
    registry
        .execute(
            &mut document,
            &format!("Pipe {source} 1 2 Cap=None ShapeBlending=Global"),
        )
        .unwrap();
    let Geometry::NurbsSurface(global) = document.objects().last().unwrap().geometry() else {
        panic!("open global pipe should be a NURBS surface")
    };
    assert_eq!(global.degree_v(), 1);
    let sample = global
        .evaluate(*global.domain_u().start(), 0.2 * *global.domain_v().end())
        .unwrap();
    assert!((sample.z() - 1.0).abs() < 1e-10);
    assert!((sample.x().hypot(sample.y()) - 1.2).abs() < 1e-10);
    registry
        .execute(&mut document, &format!("Pipe {source} 1 2 Cap=Flat"))
        .unwrap();
    let Geometry::Brep(frustum) = document.objects().last().unwrap().geometry() else {
        panic!("flat tapered pipe should be a B-rep")
    };
    assert!(frustum.is_closed());
    assert_eq!(frustum.faces().len(), 3);
}

#[test]
fn straight_thick_pipe_has_annular_caps_and_signed_wall_thickness() {
    let mut document = Document::default();
    let source = document
        .add_geometry(Geometry::Line(
            LineSegment::try_new(p(0., 0., 0.), p(0., 0., 5.), Tolerance::DEFAULT).unwrap(),
        ))
        .unwrap();
    let registry = CommandRegistry::with_builtins();
    registry
        .execute(&mut document, &format!("Pipe {source} 1 WallThickness=0.5"))
        .unwrap();
    let Geometry::Brep(tube) = document.objects().last().unwrap().geometry() else {
        panic!("flat thick pipe should be a B-rep")
    };
    assert!(tube.is_closed());
    assert!(tube.is_solid());
    assert_eq!(tube.faces().len(), 4);
    let expected = std::f64::consts::PI * (1.5_f64.powi(2) - 1.0) * 5.0;
    assert!((tube.signed_volume(Tolerance::DEFAULT).unwrap() - expected).abs() < 1e-6);

    registry
        .execute(
            &mut document,
            &format!("Pipe {source} 1 2 WallThickness=-0.25 Cap=Flat"),
        )
        .unwrap();
    let Geometry::Brep(tapered) = document.objects().last().unwrap().geometry() else {
        panic!("flat tapered thick pipe should be a B-rep")
    };
    assert!(tapered.is_closed());
    assert!(tapered.is_solid());
    assert_eq!(tapered.faces().len(), 4);
    let frustum_volume =
        |a: Real, b: Real| std::f64::consts::PI * 5.0 / 3.0 * (a * a + a * b + b * b);
    let expected = frustum_volume(1.0, 2.0) - frustum_volume(0.75, 1.75);
    let measured = tapered.signed_volume(Tolerance::DEFAULT).unwrap();
    assert!(
        (measured - expected).abs() / expected < 1e-8,
        "{measured} vs {expected}"
    );
    registry
        .execute(
            &mut document,
            &format!("Pipe {source} 1 2 WallThickness=0.25 Cap=None"),
        )
        .unwrap();
    let Geometry::Brep(open) = document.objects().last().unwrap().geometry() else {
        panic!("open thick pipe should be a B-rep")
    };
    assert!(!open.is_closed());
    assert_eq!(open.faces().len(), 2);
}

#[test]
fn circle_rail_makes_closed_torus_and_preserves_source() {
    let mut document = Document::default();
    let circle = Circle3::try_from_frame(
        p(4., -2., 1.),
        3.,
        UnitVector3::try_new(1., 0., 0., Tolerance::DEFAULT).unwrap(),
        UnitVector3::try_new(0., 0., 1., Tolerance::DEFAULT).unwrap(),
        Tolerance::DEFAULT,
    )
    .unwrap();
    let source = document.add_geometry(Geometry::Circle(circle)).unwrap();
    document
        .select_object(source, SelectionMode::Replace)
        .unwrap();
    let registry = CommandRegistry::with_builtins();
    registry.execute(&mut document, "Pipe 0.5").unwrap();
    let Geometry::NurbsSurface(surface) = document.objects().last().unwrap().geometry() else {
        panic!("circle rail should create a torus surface")
    };
    assert!(surface.is_closed_u().unwrap());
    assert!(surface.is_closed_v().unwrap());
    assert_eq!(document.objects().count(), 2);
    registry
        .execute(
            &mut document,
            &format!("Pipe {source} 0.5 WallThickness=0.2"),
        )
        .unwrap();
    let Geometry::Brep(thick) = document.objects().last().unwrap().geometry() else {
        panic!("thick circular pipe should be a B-rep")
    };
    assert!(thick.is_closed());
    assert!(thick.is_solid());
    assert_eq!(thick.faces().len(), 2);
    let expected = 2.0 * std::f64::consts::PI.powi(2) * 3.0 * (0.7_f64.powi(2) - 0.5_f64.powi(2));
    let measured = thick.signed_volume(Tolerance::DEFAULT).unwrap();
    assert!(
        (measured - expected).abs() / expected < 1e-8,
        "{measured} vs {expected}"
    );
}

#[test]
fn smooth_arc_rail_builds_capped_and_open_pipe() {
    let mut document = Document::default();
    let diagonal = 5. * std::f64::consts::FRAC_1_SQRT_2;
    let arc = CircularArc3::try_from_three_points(
        p(5., 0., 0.),
        p(diagonal, diagonal, 0.),
        p(0., 5., 0.),
        Tolerance::DEFAULT,
    )
    .unwrap();
    let source = document.add_geometry(Geometry::Arc(arc)).unwrap();
    let registry = CommandRegistry::with_builtins();
    registry
        .execute(&mut document, &format!("Pipe {source} 0.5"))
        .unwrap();
    let Geometry::Brep(capped) = document.objects().last().unwrap().geometry() else {
        panic!("arc pipe should be a B-rep")
    };
    assert!(capped.is_closed());
    assert!(capped.is_solid());
    let expected = 0.5 * std::f64::consts::PI.powi(2) * 5.0 * 0.5_f64.powi(2);
    let measured = capped.signed_volume(Tolerance::DEFAULT).unwrap();
    assert!(
        (measured - expected).abs() / expected < 1e-8,
        "{measured} vs {expected}"
    );
    let wall = capped.faces()[0].surface();
    let u = (*wall.domain_u().start() + *wall.domain_u().end()) * 0.5;
    let v = (*wall.domain_v().start() + *wall.domain_v().end()) * 0.5;
    let sample = wall.evaluate(u, v).unwrap();
    let rail = CurveRef::Arc(&arc);
    let nearest = rail
        .evaluate(rail.closest_parameter(sample, Tolerance::DEFAULT).unwrap())
        .unwrap();
    assert!((sample.distance_to(nearest).unwrap() - 0.5).abs() < 5e-3);
    registry
        .execute(
            &mut document,
            &format!("Pipe {source} 0.5 0.8 Cap=None ShapeBlending=Global"),
        )
        .unwrap();
    let Geometry::Brep(open) = document.objects().last().unwrap().geometry() else {
        panic!("arc pipe should be a B-rep")
    };
    assert!(!open.is_closed());
    let wall = open.faces()[0].surface();
    let u = (*wall.domain_u().start() + *wall.domain_u().end()) * 0.5;
    let v = (*wall.domain_v().start() + *wall.domain_v().end()) * 0.5;
    let sample = wall.evaluate(u, v).unwrap();
    let nearest = rail
        .evaluate(rail.closest_parameter(sample, Tolerance::DEFAULT).unwrap())
        .unwrap();
    assert!((sample.distance_to(nearest).unwrap() - 0.65).abs() < 5e-3);
    registry
        .execute(
            &mut document,
            &format!("Pipe {source} 0.5 WallThickness=0.2 Cap=Flat"),
        )
        .unwrap();
    let Geometry::Brep(thick) = document.objects().last().unwrap().geometry() else {
        panic!("thick arc pipe should be a B-rep")
    };
    assert!(thick.is_closed());
    assert!(thick.is_solid());
    // Two walls and one planar annular face at each end, as in Rhino.
    assert_eq!(thick.faces().len(), 4);
    let expected = 0.5 * std::f64::consts::PI.powi(2) * 5.0 * (0.7_f64.powi(2) - 0.5_f64.powi(2));
    let measured = thick.signed_volume(Tolerance::DEFAULT).unwrap();
    assert!(
        (measured - expected).abs() / expected < 1e-8,
        "{measured} vs {expected}"
    );
    registry
        .execute(&mut document, &format!("Pipe {source} 0.5 Cap=Round"))
        .unwrap();
    let Geometry::Brep(round) = document.objects().last().unwrap().geometry() else {
        panic!("round arc pipe should be a B-rep")
    };
    assert!(round.is_closed());
    assert!(round.is_solid());
    let expected = 0.5 * std::f64::consts::PI.powi(2) * 5.0 * 0.5_f64.powi(2)
        + 4.0 * std::f64::consts::PI * 0.5_f64.powi(3) / 3.0;
    let measured = round.signed_volume(Tolerance::DEFAULT).unwrap();
    assert!(
        (measured - expected).abs() / expected < 1e-8,
        "{measured} vs {expected}"
    );
    registry
        .execute(
            &mut document,
            &format!("Pipe {source} 0.5 WallThickness=0.2 Cap=Round"),
        )
        .unwrap();
    let Geometry::Brep(round_thick) = document.objects().last().unwrap().geometry() else {
        panic!("round thick arc pipe should be a B-rep")
    };
    assert!(round_thick.is_closed());
    assert!(round_thick.is_solid());
    let expected = 0.5 * std::f64::consts::PI.powi(2) * 5.0 * (0.7_f64.powi(2) - 0.5_f64.powi(2));
    let measured = round_thick.signed_volume(Tolerance::DEFAULT).unwrap();
    assert!(
        (measured - expected).abs() / expected < 1e-8,
        "{measured} vs {expected}"
    );
}

#[test]
fn tapered_arc_round_pipe_matches_rhino_volume() {
    let mut document = Document::default();
    let diagonal = 5.0 * std::f64::consts::FRAC_1_SQRT_2;
    let arc = CircularArc3::try_from_three_points(
        p(5.0, 0.0, 0.0),
        p(diagonal, diagonal, 0.0),
        p(0.0, 5.0, 0.0),
        Tolerance::DEFAULT,
    )
    .unwrap();
    let source = document.add_geometry(Geometry::Arc(arc)).unwrap();
    let registry = CommandRegistry::with_builtins();
    for (blend, reference_volume) in [("Local", 12.02855252569316), ("Global", 11.891434174037984)]
    {
        registry
            .execute(
                &mut document,
                &format!("Pipe {source} 0.5 0.8 Cap=Round ShapeBlending={blend}"),
            )
            .unwrap();
        let Geometry::Brep(pipe) = document.objects().last().unwrap().geometry() else {
            panic!("tapered arc round pipe should be a B-rep")
        };
        assert!(pipe.is_closed());
        assert!(pipe.is_solid());
        let measured = pipe.signed_volume(Tolerance::DEFAULT).unwrap();
        assert!(
            (measured - reference_volume).abs() < 5e-6,
            "{blend}: {measured} vs {reference_volume}"
        );
    }
}

#[test]
fn spatial_nurbs_rail_builds_closed_thick_pipe() {
    let mut document = Document::default();
    let registry = CommandRegistry::with_builtins();
    registry
        .execute(
            &mut document,
            "InterpCrv 0,0,0 2,1,1 4,0,2 6,-1,3 Knots=Chord Close=Open",
        )
        .unwrap();
    let source = document.objects().next().unwrap().id();
    registry
        .execute(
            &mut document,
            &format!("Pipe {source} 0.3 WallThickness=0.1 Cap=Flat"),
        )
        .unwrap();
    let Geometry::Brep(thick) = document.objects().last().unwrap().geometry() else {
        panic!("spatial thick pipe should be a B-rep")
    };
    assert!(thick.is_solid());
    assert!(thick.signed_volume(Tolerance::DEFAULT).unwrap() > 0.0);
    registry
        .execute(
            &mut document,
            &format!("Pipe {source} 0.3 WallThickness=0.1 Cap=Round"),
        )
        .unwrap();
    let Geometry::Brep(round) = document.objects().last().unwrap().geometry() else {
        panic!("spatial round thick pipe should be a B-rep")
    };
    assert!(round.is_closed());
    assert!(round.is_solid());
    assert!(round.signed_volume(Tolerance::DEFAULT).unwrap() > 0.0);
    registry
        .execute(
            &mut document,
            &format!("Pipe {source} 0.3 0.4 Cap=Round ShapeBlending=Global"),
        )
        .unwrap();
    let Geometry::Brep(tapered) = document.objects().last().unwrap().geometry() else {
        panic!("spatial tapered pipe should be a B-rep")
    };
    assert!(tapered.is_closed());
    assert!(tapered.is_solid());
    assert!(tapered.signed_volume(Tolerance::DEFAULT).unwrap() > 0.0);
}

#[test]
fn straight_local_radius_station_matches_rhino_flat_and_thick_pipes() {
    let mut document = Document::default();
    let source = document
        .add_geometry(Geometry::Line(
            LineSegment::try_new(p(0., 0., 0.), p(0., 0., 5.), Tolerance::DEFAULT).unwrap(),
        ))
        .unwrap();
    let registry = CommandRegistry::with_builtins();
    for (suffix, expected, faces) in [
        ("", 37.250313352254224, 3),
        ("WallThickness=0.5", 27.488936058139387, 4),
    ] {
        registry
            .execute(
                &mut document,
                &format!("Pipe {source} 1 Stations=0.5:2 Cap=Flat {suffix}"),
            )
            .unwrap();
        let Geometry::Brep(pipe) = document.objects().last().unwrap().geometry() else {
            panic!("station pipe should be a B-rep")
        };
        assert!(pipe.is_closed());
        assert!(pipe.is_solid());
        assert_eq!(pipe.faces().len(), faces);
        let measured = pipe.signed_volume(Tolerance::DEFAULT).unwrap();
        assert!(
            (measured - expected).abs() < 1e-6,
            "{measured} vs {expected}"
        );
    }
    registry
        .execute(
            &mut document,
            &format!("Pipe {source} 1 Stations=0.5:2 Cap=None"),
        )
        .unwrap();
    let Geometry::NurbsSurface(surface) = document.objects().last().unwrap().geometry() else {
        panic!("open station pipe should be a surface")
    };
    for (z, radius) in [(0.5, 1.104), (2.5, 2.0), (4.5, 1.104)] {
        let point = surface.evaluate(z, *surface.domain_v().start()).unwrap();
        assert!((point.x().hypot(point.y()) - radius).abs() < 1e-10);
    }
    registry
        .execute(
            &mut document,
            &format!("Pipe {source} 1 Stations=0.25:1.5,0.75:2 Cap=None"),
        )
        .unwrap();
    let Geometry::NurbsSurface(surface) = document.objects().last().unwrap().geometry() else {
        panic!("multi-station pipe should be a surface")
    };
    for (z, radius) in [(1.25, 1.5), (3.75, 2.0), (5.0, 1.0)] {
        let point = surface.evaluate(z, *surface.domain_v().start()).unwrap();
        assert!((point.x().hypot(point.y()) - radius).abs() < 1e-10);
    }
    registry
        .execute(
            &mut document,
            &format!("Pipe {source} 1 Stations=0.5:2 Cap=Round"),
        )
        .unwrap();
    let Geometry::Brep(round) = document.objects().last().unwrap().geometry() else {
        panic!("round station pipe should be a B-rep")
    };
    assert!(round.is_closed());
    assert!(round.is_solid());
    let expected = 41.439103495320765;
    let measured = round.signed_volume(Tolerance::DEFAULT).unwrap();
    assert!(
        (measured - expected).abs() < 1e-6,
        "{measured} vs {expected}"
    );
}

#[test]
fn straight_local_stations_match_rhino_volumes() {
    let mut document = Document::default();
    let source = document
        .add_geometry(Geometry::Line(
            LineSegment::try_new(p(0., 0., 0.), p(0., 0., 5.), Tolerance::DEFAULT).unwrap(),
        ))
        .unwrap();
    let registry = CommandRegistry::with_builtins();
    for (end_radius, stations, expected) in [
        (1.0, "0.5:2", 37.250313352254224),
        (1.2, "0.25:1.5,0.75:2", 40.99960133023945),
        (2.0, "0.25:1.4,0.75:1.8", 40.1960120376526),
    ] {
        registry
            .execute(
                &mut document,
                &format!(
                    "Pipe {source} 1 {end_radius} Stations={stations} ShapeBlending=Local Cap=Flat"
                ),
            )
            .unwrap();
        let Geometry::Brep(pipe) = document.objects().last().unwrap().geometry() else {
            panic!("station pipe should be a B-rep")
        };
        assert!(pipe.is_closed(), "{stations}");
        assert!(pipe.is_solid(), "{stations}");
        assert_eq!(pipe.faces().len(), 3, "{stations}");
        let measured = pipe.signed_volume(Tolerance::DEFAULT).unwrap();
        assert!(
            (measured - expected).abs() < 5e-6,
            "{stations}: {measured} vs {expected}"
        );
    }
    registry
        .execute(
            &mut document,
            &format!("Pipe {source} 1 1.2 Stations=0.25:1.5,0.75:2 Cap=None"),
        )
        .unwrap();
    let Geometry::NurbsSurface(surface) = document.objects().last().unwrap().geometry() else {
        panic!("open station pipe should be a surface")
    };
    assert_eq!(surface.degree_u(), 3);
    assert_eq!(surface.control_point_count_u(), 10);
    for (index, expected) in [
        1.0,
        0.994613743,
        1.276931283,
        1.538819894,
        1.577445556,
        1.88731348,
        2.107178385,
        1.629006755,
        1.194198649,
        1.2,
    ]
    .into_iter()
    .enumerate()
    {
        let point = surface.control_point(index, 0).unwrap().point();
        assert!(
            (point.x().hypot(point.y()) - expected).abs() < 2e-5,
            "control {index}: {point:?} vs radius {expected}"
        );
    }
}

#[test]
fn arc_local_radius_station_matches_rhino_flat_pipe() {
    let mut document = Document::default();
    let diagonal = 5.0 * std::f64::consts::FRAC_1_SQRT_2;
    let arc = CircularArc3::try_from_three_points(
        p(5.0, 0.0, 0.0),
        p(diagonal, diagonal, 0.0),
        p(0.0, 5.0, 0.0),
        Tolerance::DEFAULT,
    )
    .unwrap();
    let source = document.add_geometry(Geometry::Arc(arc)).unwrap();
    CommandRegistry::with_builtins()
        .execute(
            &mut document,
            &format!("Pipe {source} 0.5 Stations=0.5:0.8 Cap=Flat"),
        )
        .unwrap();
    let Geometry::Brep(pipe) = document.objects().last().unwrap().geometry() else {
        panic!("arc station pipe should be a B-rep")
    };
    assert!(pipe.is_closed());
    assert!(pipe.is_solid());
    assert_eq!(pipe.faces().len(), 3);
    let measured = pipe.signed_volume(Tolerance::DEFAULT).unwrap();
    let expected = 10.694421898601524;
    assert!(
        (measured - expected).abs() < 5e-6,
        "{measured} vs {expected}"
    );
}

#[test]
fn bezier_rail_pipe_matches_rhino_flat_volumes() {
    let mut document = Document::default();
    let rail = NurbsCurve::try_new(
        3,
        vec![p(0., 0., 0.), p(2., 0., 0.), p(3., 2., 1.), p(5., 0., 2.)],
        vec![0., 0., 0., 0., 1., 1., 1., 1.],
    )
    .unwrap();
    let source = document.add_geometry(Geometry::NurbsCurve(rail)).unwrap();
    let registry = CommandRegistry::with_builtins();
    for (suffix, expected) in [
        ("", 1.6495215095113431),
        ("0.5", 3.021505338976187),
        ("0.5 ShapeBlending=Global", 2.9935770434779623),
    ] {
        registry
            .execute(
                &mut document,
                &format!("Pipe {source} 0.3 {suffix} Cap=Flat"),
            )
            .unwrap();
        let Geometry::Brep(pipe) = document.objects().last().unwrap().geometry() else {
            panic!("Bézier pipe should be a B-rep")
        };
        assert!(pipe.is_closed());
        assert!(pipe.is_solid());
        let measured = pipe.signed_volume(Tolerance::DEFAULT).unwrap();
        assert!(
            (measured - expected).abs() < 5e-6,
            "{suffix}: {measured} vs {expected}"
        );
    }
}

#[test]
fn multispan_rail_pipe_matches_rhino_flat_volumes() {
    let mut document = Document::default();
    let rail = NurbsCurve::try_new(
        3,
        vec![
            p(0., 0., 0.),
            p(1., 1., 0.),
            p(2., -1., 1.),
            p(3., 2., 1.),
            p(5., 0., 2.),
        ],
        vec![0., 0., 0., 0., 0.5, 1., 1., 1., 1.],
    )
    .unwrap();
    let source = document.add_geometry(Geometry::NurbsCurve(rail)).unwrap();
    let registry = CommandRegistry::with_builtins();
    for (suffix, expected) in [("", 1.7380221094021495), ("0.5", 3.183615713670431)] {
        registry
            .execute(
                &mut document,
                &format!("Pipe {source} 0.3 {suffix} Cap=Flat"),
            )
            .unwrap();
        let Geometry::Brep(pipe) = document.objects().last().unwrap().geometry() else {
            panic!("multispan pipe should be a B-rep")
        };
        assert!(pipe.is_closed());
        assert!(pipe.is_solid());
        let measured = pipe.signed_volume(Tolerance::DEFAULT).unwrap();
        assert!(
            (measured - expected).abs() < 5e-6,
            "{suffix}: {measured} vs {expected}"
        );
    }
}

#[test]
fn tangent_line_arc_pipe_matches_rhino_fit_rail_modes() {
    let mut document = Document::default();
    let diagonal = 2.0_f64.sqrt();
    let line = LineSegment::try_new(p(0., 0., 0.), p(2., 0., 0.), Tolerance::DEFAULT).unwrap();
    let arc = CircularArc3::try_from_three_points(
        p(2., 0., 0.),
        p(2. + diagonal, 2. - diagonal, 0.),
        p(4., 2., 0.),
        Tolerance::DEFAULT,
    )
    .unwrap();
    let rail =
        PolyCurve3::try_new(vec![CurveSegment3::Line(line), CurveSegment3::Arc(arc)]).unwrap();
    let source = document.add_geometry(Geometry::PolyCurve(rail)).unwrap();
    let registry = CommandRegistry::with_builtins();
    for (options, expected, faces) in [
        ("", 1.4537510629312786, 4),
        ("FitRail=Yes", 1.4537510834790615, 3),
        ("0.5", 2.5796673434153528, 4),
        ("0.5 FitRail=Yes", 2.6629032304651, 3),
        ("0.5 ShapeBlending=Global", 2.5719585204790043, 4),
        (
            "0.5 ShapeBlending=Global FitRail=Yes",
            2.6382894558171768,
            3,
        ),
    ] {
        registry
            .execute(
                &mut document,
                &format!("Pipe {source} 0.3 {options} Cap=Flat"),
            )
            .unwrap();
        let Geometry::Brep(pipe) = document.objects().last().unwrap().geometry() else {
            panic!("polycurve pipe should be a B-rep")
        };
        assert!(pipe.is_closed());
        assert!(pipe.is_solid());
        assert_eq!(pipe.faces().len(), faces);
        let measured = pipe.signed_volume(Tolerance::DEFAULT).unwrap();
        assert!(
            (measured - expected).abs() < 5e-6,
            "{options}: {measured} vs {expected}"
        );
    }
    for (options, expected, faces) in [
        ("Cap=Round", 2.8980153924831518, 4),
        ("WallThickness=0.2 Cap=Flat", 3.1964619294525423, 6),
    ] {
        registry
            .execute(&mut document, &format!("Pipe {source} 0.3 0.5 {options}"))
            .unwrap();
        let Geometry::Brep(pipe) = document.objects().last().unwrap().geometry() else {
            panic!("polycurve pipe should be a B-rep")
        };
        assert!(pipe.is_closed());
        assert!(pipe.is_solid());
        assert_eq!(pipe.faces().len(), faces);
        let measured = pipe.signed_volume(Tolerance::DEFAULT).unwrap();
        assert!(
            (measured - expected).abs() < 5e-6,
            "{options}: {measured} vs {expected}"
        );
    }
    for (options, expected) in [
        ("Stations=0.25:0.65", 5.1070735157919955),
        ("Stations=0.38898452964834274:0.65", 4.801208532983822),
    ] {
        registry
            .execute(
                &mut document,
                &format!("Pipe {source} 0.3 0.5 {options} Cap=Flat"),
            )
            .unwrap();
        let Geometry::Brep(pipe) = document.objects().last().unwrap().geometry() else {
            panic!("polycurve station pipe should be a B-rep")
        };
        assert!(pipe.is_closed(), "{options}");
        assert!(pipe.is_solid(), "{options}");
        assert_eq!(pipe.faces().len(), 4, "{options}");
        let measured = pipe.signed_volume(Tolerance::DEFAULT).unwrap();
        assert!(
            (measured - expected).abs() < 5e-6,
            "{options}: {measured} vs {expected}"
        );
    }
    let count = document.objects().count();
    assert!(
        registry
            .execute(
                &mut document,
                &format!("Pipe {source} 0.3 0.5 Stations=0.5:0.65 Cap=Flat"),
            )
            .is_err()
    );
    assert_eq!(document.objects().count(), count);
    assert!(
        registry
            .execute(
                &mut document,
                &format!("Pipe {source} 0.3 0.5 Stations=0.5:0.65 FitRail=Yes Cap=Flat"),
            )
            .is_err()
    );
    assert_eq!(document.objects().count(), count);
}

#[test]
fn sharp_two_line_pipe_matches_rhino_miter() {
    let mut document = Document::default();
    let first = LineSegment::try_new(p(0., 0., 0.), p(2., 0., 0.), Tolerance::DEFAULT).unwrap();
    let second = LineSegment::try_new(p(2., 0., 0.), p(2., 2., 0.), Tolerance::DEFAULT).unwrap();
    let rail = PolyCurve3::try_new(vec![
        CurveSegment3::Line(first),
        CurveSegment3::Line(second),
    ])
    .unwrap();
    let source = document.add_geometry(Geometry::PolyCurve(rail)).unwrap();
    let registry = CommandRegistry::with_builtins();
    for (options, expected, faces) in [
        ("Cap=Flat", 1.1309737513007323, 4),
        ("Cap=Round", 1.2440711643189903, 4),
        ("WallThickness=0.2 Cap=Flat", 2.010619896220259, 6),
    ] {
        registry
            .execute(&mut document, &format!("Pipe {source} 0.3 {options}"))
            .unwrap();
        let Geometry::Brep(pipe) = document.objects().last().unwrap().geometry() else {
            panic!("mitered pipe should be a B-rep")
        };
        assert!(pipe.is_closed(), "{options}");
        assert!(pipe.is_solid(), "{options}");
        assert_eq!(pipe.faces().len(), faces, "{options}");
        let measured = pipe.signed_volume(Tolerance::DEFAULT).unwrap();
        assert!(
            (measured - expected).abs() < 5e-6,
            "{options}: {measured} vs {expected}"
        );
    }
}

#[test]
fn oblique_three_dimensional_miter_matches_rhino_volume() {
    let mut document = Document::default();
    let first = LineSegment::try_new(p(0., 0., 0.), p(2., 0., 0.), Tolerance::DEFAULT).unwrap();
    let second = LineSegment::try_new(p(2., 0., 0.), p(3., 1., 1.), Tolerance::DEFAULT).unwrap();
    let rail = PolyCurve3::try_new(vec![
        CurveSegment3::Line(first),
        CurveSegment3::Line(second),
    ])
    .unwrap();
    let source = document.add_geometry(Geometry::PolyCurve(rail)).unwrap();
    let registry = CommandRegistry::with_builtins();
    for (cap, expected) in [("Flat", 1.0552124780965215), ("Round", 1.1683098049457938)] {
        registry
            .execute(&mut document, &format!("Pipe {source} 0.3 Cap={cap}"))
            .unwrap();
        let Geometry::Brep(pipe) = document.objects().last().unwrap().geometry() else {
            panic!("oblique pipe should be a B-rep")
        };
        assert!(pipe.is_closed(), "{cap}");
        assert!(pipe.is_solid(), "{cap}");
        let measured = pipe.signed_volume(Tolerance::DEFAULT).unwrap();
        assert!(
            (measured - expected).abs() < 5e-6,
            "{cap}: {measured} vs {expected}"
        );
    }
}

#[test]
fn sharp_polyline_pipe_matches_rhino_miter() {
    let mut document = Document::default();
    let rail = Polyline3::try_new(
        vec![p(0., 0., 0.), p(2., 0., 0.), p(2., 2., 0.)],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let source = document.add_geometry(Geometry::Polyline(rail)).unwrap();
    let registry = CommandRegistry::with_builtins();
    for (cap, expected) in [("Flat", 1.1309737513007323), ("Round", 1.2440711643189903)] {
        registry
            .execute(&mut document, &format!("Pipe {source} 0.3 Cap={cap}"))
            .unwrap();
        let Geometry::Brep(pipe) = document.objects().last().unwrap().geometry() else {
            panic!("mitered polyline pipe should be a B-rep")
        };
        assert!(pipe.is_closed(), "{cap}");
        assert!(pipe.is_solid(), "{cap}");
        assert_eq!(pipe.faces().len(), 4, "{cap}");
        let measured = pipe.signed_volume(Tolerance::DEFAULT).unwrap();
        assert!(
            (measured - expected).abs() < 5e-6,
            "{cap}: {measured} vs {expected}"
        );
    }
}

#[test]
fn three_segment_polyline_pipe_matches_rhino_miters() {
    let mut document = Document::default();
    let rail = Polyline3::try_new(
        vec![p(0., 0., 0.), p(2., 0., 0.), p(2., 2., 0.), p(4., 2., 0.)],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let source = document.add_geometry(Geometry::Polyline(rail)).unwrap();
    let registry = CommandRegistry::with_builtins();
    for (options, expected, faces) in [
        ("Cap=Flat", 1.6964605759770885, 5),
        ("Cap=Round", 1.8095579066239338, 5),
        ("WallThickness=0.2 Cap=Flat", 3.0159299051156543, 8),
    ] {
        registry
            .execute(&mut document, &format!("Pipe {source} 0.3 {options}"))
            .unwrap();
        let Geometry::Brep(pipe) = document.objects().last().unwrap().geometry() else {
            panic!("three-segment pipe should be a B-rep")
        };
        assert!(pipe.is_closed(), "{options}");
        assert!(pipe.is_solid(), "{options}");
        assert_eq!(pipe.faces().len(), faces, "{options}");
        let measured = pipe.signed_volume(Tolerance::DEFAULT).unwrap();
        assert!(
            (measured - expected).abs() < 5e-6,
            "{options}: {measured} vs {expected}"
        );
    }
}

#[test]
fn spatial_three_segment_polyline_pipe_matches_rhino_miters() {
    let mut document = Document::default();
    let rail = Polyline3::try_new(
        vec![p(0., 0., 0.), p(2., 0., 0.), p(2., 2., 0.), p(2., 2., 2.)],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let source = document.add_geometry(Geometry::Polyline(rail)).unwrap();
    let registry = CommandRegistry::with_builtins();
    for (options, expected, faces) in [
        ("Cap=Flat", 1.6964609638052046, 5),
        ("Cap=Round", 1.8095583636292623, 5),
        ("WallThickness=0.2 Cap=Flat", 3.0159304945160126, 8),
    ] {
        registry
            .execute(&mut document, &format!("Pipe {source} 0.3 {options}"))
            .unwrap();
        let Geometry::Brep(pipe) = document.objects().last().unwrap().geometry() else {
            panic!("spatial three-segment pipe should be a B-rep")
        };
        assert!(pipe.is_closed(), "{options}");
        assert!(pipe.is_solid(), "{options}");
        assert_eq!(pipe.faces().len(), faces, "{options}");
        let measured = pipe.signed_volume(Tolerance::DEFAULT).unwrap();
        assert!(
            (measured - expected).abs() < 5e-6,
            "{options}: {measured} vs {expected}"
        );
    }
}

#[test]
fn skew_spatial_polyline_pipe_matches_rhino_volume() {
    let mut document = Document::default();
    let rail = Polyline3::try_new(
        vec![
            p(0., 0., 0.),
            p(2., 0., 0.),
            p(3., 1., 0.5),
            p(4., 1.3, 1.8),
        ],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let source = document.add_geometry(Geometry::Polyline(rail)).unwrap();
    let registry = CommandRegistry::with_builtins();
    for (options, expected) in [
        ("Cap=Flat", 1.461029031976258),
        ("Cap=Round", 1.574126366134179),
        ("WallThickness=0.2 Cap=Flat", 2.5973849459504827),
    ] {
        registry
            .execute(&mut document, &format!("Pipe {source} 0.3 {options}"))
            .unwrap();
        let Geometry::Brep(pipe) = document.objects().last().unwrap().geometry() else {
            panic!("skew spatial pipe should be a B-rep")
        };
        assert!(pipe.is_closed(), "{options}");
        assert!(pipe.is_solid(), "{options}");
        let measured = pipe.signed_volume(Tolerance::DEFAULT).unwrap();
        assert!(
            (measured - expected).abs() < 5e-6,
            "{options}: {measured} vs {expected}"
        );
    }
}

#[test]
fn fitted_sharp_constant_radius_pipes_match_rhino_volume() {
    let tolerance = Tolerance::DEFAULT;
    let first = LineSegment::try_new(p(0., 0., 0.), p(2., 0., 0.), tolerance).unwrap();
    let second = LineSegment::try_new(p(2., 0., 0.), p(2., 2., 0.), tolerance).unwrap();
    let two_lines = PolyCurve3::try_new(vec![
        CurveSegment3::Line(first),
        CurveSegment3::Line(second),
    ])
    .unwrap();
    let rails = [
        (
            "two lines",
            Geometry::PolyCurve(two_lines),
            [1.130973357832259, 1.2440706907864723, 2.0106193005915816],
        ),
        (
            "planar polyline",
            Geometry::Polyline(
                Polyline3::try_new(
                    vec![p(0., 0., 0.), p(2., 0., 0.), p(2., 2., 0.), p(4., 2., 0.)],
                    tolerance,
                )
                .unwrap(),
            ),
            [1.696460042103737, 1.8095573727505831, 3.0159289603031096],
        ),
        (
            "orthogonal spatial polyline",
            Geometry::Polyline(
                Polyline3::try_new(
                    vec![p(0., 0., 0.), p(2., 0., 0.), p(2., 2., 0.), p(2., 2., 2.)],
                    tolerance,
                )
                .unwrap(),
            ),
            [1.696460048805896, 1.8095573844488695, 3.0159289406129766],
        ),
        (
            "skew spatial polyline",
            Geometry::Polyline(
                Polyline3::try_new(
                    vec![
                        p(0., 0., 0.),
                        p(2., 0., 0.),
                        p(3., 1., 0.5),
                        p(4., 1.3, 1.8),
                    ],
                    tolerance,
                )
                .unwrap(),
            ),
            [1.4610290445769174, 1.5741263764973636, 2.5973849605325876],
        ),
    ];
    let registry = CommandRegistry::with_builtins();
    let options = ["Cap=Flat", "Cap=Round", "WallThickness=0.2 Cap=Flat"];
    for (name, rail, expected) in rails {
        let mut document = Document::default();
        let source = document.add_geometry(rail).unwrap();
        for (option, volume) in options.into_iter().zip(expected) {
            registry
                .execute(
                    &mut document,
                    &format!("Pipe {source} 0.3 FitRail=Yes {option}"),
                )
                .unwrap();
            let Geometry::Brep(pipe) = document.objects().last().unwrap().geometry() else {
                panic!("{name}: fitted pipe should be a B-rep")
            };
            assert!(pipe.is_closed(), "{name}: {option}");
            assert!(pipe.is_solid(), "{name}: {option}");
            let measured = pipe.signed_volume(tolerance).unwrap();
            assert!(
                (measured - volume).abs() < 5e-6,
                "{name}: {option}: {measured} vs {volume}"
            );
        }
    }
}

#[test]
fn tapered_sharp_fit_profile_matches_rhino_volume() {
    let tolerance = Tolerance::DEFAULT;
    let mut mismatches = Vec::new();
    for (vertices, expected) in [
        (
            vec![p(0., 0., 0.), p(2., 0., 0.), p(2., 2., 0.)],
            [
                2.0716563894655327,
                2.0525075453971313,
                2.3900044399127722,
                2.5132743510996836,
                2.5132743406284277,
            ],
        ),
        (
            vec![p(0., 0., 0.), p(2., 0., 0.), p(3., 1., 1.)],
            [
                1.9328814905368568,
                1.9150153216013213,
                2.251229540274963,
                2.34491674771564,
                2.3449167040785506,
            ],
        ),
        (
            vec![
                p(0., 0., 0.),
                p(2., 0., 0.),
                p(3., 1., 0.5),
                p(4., 1.3, 1.8),
            ],
            [
                2.676234466580245,
                2.6514974679478795,
                2.9945825138115723,
                3.2467313600092877,
                3.2467315788085314,
            ],
        ),
    ] {
        let lines = vertices
            .windows(2)
            .map(|pair| LineSegment::try_new(pair[0], pair[1], tolerance).unwrap())
            .collect::<Vec<_>>();
        let options = [
            (SweepBlend::Local, PipeCap::Flat, None),
            (SweepBlend::Global, PipeCap::Flat, None),
            (SweepBlend::Local, PipeCap::Round, None),
            (SweepBlend::Local, PipeCap::Flat, Some(0.2)),
            (SweepBlend::Global, PipeCap::Flat, Some(0.2)),
        ];
        for ((blend, cap, thickness), expected) in options.into_iter().zip(expected) {
            let pipe = mitered_line_pipe_profile(
                &lines,
                [0.3, 0.5],
                blend,
                cap,
                thickness,
                false,
                tolerance,
            )
            .unwrap()
            .unwrap();
            assert!(pipe.is_closed());
            assert!(pipe.is_solid());
            let measured = pipe.signed_volume(tolerance).unwrap();
            if (measured - expected).abs() >= 5e-6 {
                mismatches.push(format!(
                    "{blend:?} {cap:?} {thickness:?}: {measured} vs {expected}"
                ));
            }
        }
    }
    assert!(mismatches.is_empty(), "{}", mismatches.join("\n"));
}

#[test]
fn fitted_tapered_miter_tracks_rhino_wall_at_matched_axial_positions() {
    let tolerance = Tolerance::DEFAULT;
    let lines = [
        LineSegment::try_new(p(0., 0., 0.), p(2., 0., 0.), tolerance).unwrap(),
        LineSegment::try_new(p(2., 0., 0.), p(2., 2., 0.), tolerance).unwrap(),
    ];
    let directions = lines.map(|line| line.direction(tolerance).unwrap().as_vector());
    let seam = p(0., 0., 0.).vector_to(p(0., 0., 1.)).unwrap();
    let first_frame =
        Frame3::try_from_x_and_normal(lines[0].start(), seam, directions[0], tolerance).unwrap();
    let second_frame =
        Frame3::try_from_x_and_normal(lines[1].start(), seam, directions[1], tolerance).unwrap();
    let surfaces = [
        mitered_line_profile_surface(
            lines[0],
            first_frame,
            [0.3, 0.5],
            0.0,
            SweepBlend::Local,
            [0.0, 0.5],
            None,
            Some(directions),
            tolerance,
        )
        .unwrap(),
        mitered_line_profile_surface(
            lines[1],
            second_frame,
            [0.3, 0.5],
            0.0,
            SweepBlend::Local,
            [0.5, 1.0],
            Some(directions),
            None,
            tolerance,
        )
        .unwrap(),
    ];
    let references = [
        [
            (0.4382538993558626, 0.30859375),
            (0.8632822913683168, 0.33125),
            (1.2566695376966126, 0.36328125),
        ],
        [
            (0.7923368998234105, 0.43671875),
            (1.1936672523437442, 0.46875),
            (1.5981639786922057, 0.49140625),
        ],
    ];
    for (index, surface) in surfaces.iter().enumerate() {
        let circle_parameter = 3.0 * std::f64::consts::FRAC_PI_2;
        for (axial, expected_radius) in references[index] {
            let mut low = 0.0;
            let mut high = 2.0;
            for _ in 0..60 {
                let middle = 0.5 * (low + high);
                let point = surface.evaluate(circle_parameter, middle).unwrap();
                let measured_axial = point.to_array()[index];
                if measured_axial < axial {
                    low = middle;
                } else {
                    high = middle;
                }
            }
            let point = surface
                .evaluate(circle_parameter, 0.5 * (low + high))
                .unwrap();
            let measured_radius = if index == 0 {
                point.y()
            } else {
                2.0 - point.x()
            };
            assert!(
                (measured_radius - expected_radius).abs() < 1e-4,
                "segment {index}, axial {axial}: {measured_radius} vs {expected_radius}"
            );
        }
    }
}

#[test]
fn fitted_tapered_sharp_pipe_command_uses_miter_profile() {
    let tolerance = Tolerance::DEFAULT;
    let mut document = Document::default();
    let lines = [
        LineSegment::try_new(p(0., 0., 0.), p(2., 0., 0.), tolerance).unwrap(),
        LineSegment::try_new(p(2., 0., 0.), p(2., 2., 0.), tolerance).unwrap(),
    ];
    let rail = PolyCurve3::try_new(lines.into_iter().map(CurveSegment3::Line).collect()).unwrap();
    let source = document.add_geometry(Geometry::PolyCurve(rail)).unwrap();
    let registry = CommandRegistry::with_builtins();
    for (options, expected) in [
        ("Cap=Flat", 2.0716563894655327),
        ("ShapeBlending=Global Cap=Flat", 2.0525075453971313),
        ("Cap=Round", 2.3900044399127722),
        ("ShapeBlending=Global Cap=Round", 2.3566305015018547),
        ("WallThickness=0.2 Cap=Flat", 2.5132743510996836),
    ] {
        registry
            .execute(
                &mut document,
                &format!("Pipe {source} 0.3 0.5 FitRail=Yes {options}"),
            )
            .unwrap();
        let Geometry::Brep(pipe) = document.objects().last().unwrap().geometry() else {
            panic!("fitted tapered pipe should be a B-rep")
        };
        assert!(pipe.is_closed(), "{options}");
        assert!(pipe.is_solid(), "{options}");
        let measured = pipe.signed_volume(tolerance).unwrap();
        assert!(
            (measured - expected).abs() < 5e-6,
            "{options}: {measured} vs {expected}"
        );
    }
    let short_rail = PolyCurve3::try_new(vec![
        CurveSegment3::Line(LineSegment::try_new(p(0., 0., 0.), p(2., 0., 0.), tolerance).unwrap()),
        CurveSegment3::Line(
            LineSegment::try_new(
                p(2., 0., 0.),
                p(3., std::f64::consts::SQRT_2, 0.),
                tolerance,
            )
            .unwrap(),
        ),
    ])
    .unwrap();
    let short_source = document
        .add_geometry(Geometry::PolyCurve(short_rail))
        .unwrap();
    registry
        .execute(
            &mut document,
            &format!("Pipe {short_source} 0.3 0.5 FitRail=Yes ShapeBlending=Global Cap=Round"),
        )
        .unwrap();
    let Geometry::Brep(short_pipe) = document.objects().last().unwrap().geometry() else {
        panic!("short horizontal pipe should be a B-rep")
    };
    assert!(short_pipe.is_solid());
    let measured = short_pipe.signed_volume(tolerance).unwrap();
    assert!((measured - 2.2182048402524526).abs() < 5e-6);

    let skew = Polyline3::try_new(
        vec![
            p(0., 0., 0.),
            p(2., 0., 0.),
            p(3., 1., 0.5),
            p(4., 1.3, 1.8),
        ],
        tolerance,
    )
    .unwrap();
    let skew_id = document.add_geometry(Geometry::Polyline(skew)).unwrap();
    for (options, expected) in [
        ("Cap=Round", 2.9945825138115723),
        (
            "ShapeBlending=Global WallThickness=0.2 Cap=Flat",
            3.2467315788085314,
        ),
        (
            "ShapeBlending=Global WallThickness=0.2 Cap=Round",
            3.2467315788085314,
        ),
    ] {
        registry
            .execute(
                &mut document,
                &format!("Pipe {skew_id} 0.3 0.5 FitRail=Yes {options}"),
            )
            .unwrap();
        let Geometry::Brep(pipe) = document.objects().last().unwrap().geometry() else {
            panic!("fitted spatial polyline pipe should be a B-rep")
        };
        assert!(pipe.is_closed(), "{options}");
        assert!(pipe.is_solid(), "{options}");
        let measured = pipe.signed_volume(tolerance).unwrap();
        assert!(
            (measured - expected).abs() < 5e-6,
            "{options}: {measured} vs {expected}"
        );
    }
    let count = document.objects().count();
    assert!(
        registry
            .execute(
                &mut document,
                &format!("Pipe {skew_id} 0.3 0.5 FitRail=Yes ShapeBlending=Global Cap=Round"),
            )
            .is_err()
    );
    assert_eq!(document.objects().count(), count);
}

#[test]
fn fitted_closed_rectangle_pipe_matches_rhino_volume() {
    let tolerance = Tolerance::DEFAULT;
    let rail = Polyline3::try_new(
        vec![
            p(0., 0., 0.),
            p(3., 0., 0.),
            p(3., 2., 0.),
            p(0., 2., 0.),
            p(0., 0., 0.),
        ],
        tolerance,
    )
    .unwrap();
    let mut document = Document::default();
    let source = document.add_geometry(Geometry::Polyline(rail)).unwrap();
    let registry = CommandRegistry::with_builtins();
    for (option, expected) in [
        ("Cap=None", 2.8274333878128886),
        ("Cap=Flat", 2.8274333878128886),
        ("Cap=Round", 2.8274333878128886),
        ("WallThickness=0.2 Cap=None", 5.02654824276054),
        ("WallThickness=0.2 Cap=Flat", 5.02654824276054),
        ("WallThickness=0.2 Cap=Round", 5.02654824276054),
    ] {
        registry
            .execute(
                &mut document,
                &format!("Pipe {source} 0.3 FitRail=Yes {option}"),
            )
            .unwrap();
        let Geometry::Brep(pipe) = document.objects().last().unwrap().geometry() else {
            panic!("closed rectangular pipe should be a B-rep")
        };
        assert!(pipe.is_closed(), "{option}");
        assert!(pipe.is_solid(), "{option}");
        assert_eq!(
            pipe.faces().len(),
            if option.contains("WallThickness") {
                2
            } else {
                1
            }
        );
        let wall = pipe.faces()[0].surface();
        assert_eq!((wall.degree_u(), wall.degree_v()), (3, 2));
        assert!(wall.is_closed_u().unwrap());
        assert!(wall.is_closed_v().unwrap());
        assert_eq!(
            (wall.control_point_count_u(), wall.control_point_count_v()),
            (16, 9)
        );
        let measured = pipe.signed_volume(tolerance).unwrap();
        assert!(
            (measured - expected).abs() < 5e-6,
            "{option}: {measured} vs {expected}"
        );
    }
    let count = document.objects().count();
    assert!(
        registry
            .execute(&mut document, &format!("Pipe {source} 0.3 Cap=Flat"))
            .is_err()
    );
    assert_eq!(document.objects().count(), count);
}

#[test]
fn fitted_closed_triangle_and_spatial_loop_make_solid_pipes() {
    let tolerance = Tolerance::DEFAULT;
    let registry = CommandRegistry::with_builtins();
    for (name, vertices, rhino_volumes, planar) in [
        (
            "triangle",
            vec![p(0., 0., 0.), p(3., 0., 0.), p(1.5, 2.5, 0.), p(0., 0., 0.)],
            [2.4968928233655134, 4.438920567236832],
            true,
        ),
        (
            "spatial loop",
            vec![
                p(0., 0., 0.),
                p(3., 0., 0.),
                p(3., 2., 1.),
                p(0., 2., 0.),
                p(0., 0., 0.),
            ],
            [2.9073922697381915, 5.168697873404391],
            false,
        ),
    ] {
        let perimeter = vertices
            .windows(2)
            .map(|pair| pair[0].distance_to(pair[1]).unwrap())
            .sum::<Real>();
        let mut document = Document::default();
        let rail = Polyline3::try_new(vertices, tolerance).unwrap();
        let source = document.add_geometry(Geometry::Polyline(rail)).unwrap();
        for (index, (options, area)) in [
            ("", std::f64::consts::PI * 0.3_f64.powi(2)),
            (
                "WallThickness=0.2",
                std::f64::consts::PI * (0.5_f64.powi(2) - 0.3_f64.powi(2)),
            ),
        ]
        .into_iter()
        .enumerate()
        {
            registry
                .execute(
                    &mut document,
                    &format!("Pipe {source} 0.3 FitRail=Yes Cap=Flat {options}"),
                )
                .unwrap();
            let Geometry::Brep(pipe) = document.objects().last().unwrap().geometry() else {
                panic!("closed loop pipe should be a B-rep")
            };
            assert!(pipe.is_closed(), "{name} {options}");
            assert!(pipe.is_solid(), "{name} {options}");
            assert_eq!(pipe.faces().len(), if index == 0 { 1 } else { 2 });
            let wall = pipe.faces()[0].surface();
            assert_eq!((wall.degree_u(), wall.degree_v()), (3, 2));
            assert_eq!(wall.control_point_count_v(), 9);
            let measured = pipe.signed_volume(tolerance).unwrap();
            if planar {
                assert!(
                    (measured - perimeter * area).abs() < 5e-6,
                    "{name} {options}: {measured} vs analytic {}",
                    perimeter * area,
                );
            }
            assert!(
                (measured - rhino_volumes[index]).abs() < 5e-6,
                "{name} {options}: {measured} vs Rhino {}",
                rhino_volumes[index]
            );
        }
    }
}

#[test]
fn fitted_closed_spatial_pipe_tracks_rhino_wall_samples() {
    let tolerance = Tolerance::DEFAULT;
    let rail = Polyline3::try_new(
        vec![
            p(0., 0., 0.),
            p(3., 0., 0.),
            p(3., 2., 1.),
            p(0., 2., 0.),
            p(0., 0., 0.),
        ],
        tolerance,
    )
    .unwrap();
    let mut document = Document::default();
    let source = document.add_geometry(Geometry::Polyline(rail)).unwrap();
    CommandRegistry::with_builtins()
        .execute(
            &mut document,
            &format!("Pipe {source} 0.3 FitRail=Yes Cap=Flat"),
        )
        .unwrap();
    let Geometry::Brep(pipe) = document.objects().last().unwrap().geometry() else {
        panic!("closed spatial pipe should be a B-rep")
    };
    let samples = [
        p(2.580779172514338, 0.29944294122962345, 0.003748513965577651),
        p(
            2.905837102170516,
            0.003748513965577929,
            -0.29944294122962267,
        ),
        p(2.727387704892325, 1.0338568689426977, 0.3815722284741339),
        p(3.2726122951076744, 0.933142250924031, 0.6019273314592302),
        p(1.6303572709321312, 1.7353380686061608, 0.4019940648509084),
        p(1.5936022690395901, 2.264661931393839, 0.6726591151396656),
        p(
            0.29797433174408344,
            1.0756291590617515,
            -0.01363095987482699,
        ),
        p(
            -0.2979743317440834,
            1.1235398334008513,
            0.013630959874826981,
        ),
        p(
            0.44352697699132704,
            0.29944294122962295,
            -0.0037485139655774264,
        ),
        p(
            -0.0430983639715959,
            -0.29944294122962295,
            0.003748513965577389,
        ),
    ];
    for sample in samples {
        let nearest = pipe
            .faces()
            .iter()
            .filter_map(|face| {
                let (u, v) = face.surface().closest_parameters(sample, tolerance).ok()?;
                let candidate = face.surface().evaluate(u, v).ok()?;
                sample.distance_to(candidate).ok()
            })
            .fold(Real::INFINITY, Real::min);
        assert!(nearest < 5e-4, "{sample:?}: gap {nearest}");
    }
}

#[test]
fn invalid_inputs_leave_document_unchanged() {
    let mut document = Document::default();
    let source = document
        .add_geometry(Geometry::Line(
            LineSegment::try_new(p(0., 0., 0.), p(0., 0., 5.), Tolerance::DEFAULT).unwrap(),
        ))
        .unwrap();
    document
        .select_object(source, SelectionMode::Replace)
        .unwrap();
    let originals = document.objects().cloned().collect::<Vec<_>>();
    let registry = CommandRegistry::with_builtins();
    for command in [
        "Pipe",
        "Pipe 0",
        "Pipe -1",
        "Pipe NaN",
        "Pipe 1 Cap=Rounding",
        "Pipe 1 Cap=Flat Cap=None",
        "Pipe 1 ShapeBlending=Other",
        "Pipe 1 FitRail=Maybe",
        "Pipe 1 FitRail=Yes FitRail=No",
        "Pipe 1 WallThickness=0",
        "Pipe 1 WallThickness=-1",
        "Pipe 1 WallThickness=NaN",
        "Pipe 1 WallThickness=inf",
        "Pipe 1 Thick=Yes",
        "Pipe 1 Thick=No WallThickness=0.2",
        "Pipe 1 Stations=",
        "Pipe 1 Stations=0:2",
        "Pipe 1 Stations=1:2",
        "Pipe 1 Stations=0.5:-2",
        "Pipe 1 Stations=0.5:2,0.25:3",
        "Pipe 1 Stations=0.5:2,0.5:3",
        "Pipe 1 Stations=0.5:2 Stations=0.75:3",
        "Pipe 1 Stations=0.5:2 ShapeBlending=Global",
        "Pipe 1 Stations=0.5:0.3 WallThickness=-0.5",
    ] {
        assert!(
            registry.execute(&mut document, command).is_err(),
            "{command}"
        );
        assert_eq!(document.objects().cloned().collect::<Vec<_>>(), originals);
    }
}
