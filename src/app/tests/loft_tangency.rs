use super::*;
use viboceros_geometry::{Brep, Circle3, NurbsSurface};

fn enter(app: &mut VibocerosApp, input: &str) {
    app.command_input = input.into();
    app.run_command();
}

fn add_profile_edge(
    app: &mut VibocerosApp,
    surface: NurbsSurface,
    on_edge: impl Fn(Point3) -> bool,
) -> String {
    let brep = Brep::try_surface_face(surface, app.document.tolerance()).unwrap();
    let edge = brep
        .edges()
        .iter()
        .position(|edge| {
            let domain = edge.curve().domain();
            let midpoint = edge
                .curve()
                .evaluate(domain.start().midpoint(*domain.end()))
                .unwrap();
            on_edge(midpoint)
        })
        .unwrap();
    let id = app.document.add_geometry(Geometry::Brep(brep)).unwrap();
    format!("Edge={id},{edge}")
}

#[test]
fn loft_tangency_on_two_planes_updates_the_preview_and_keeps_the_middle_profile() {
    let mut app = test_app();
    let mut edges = Vec::new();
    for (y, z, far_y, far_z) in [(0., 0., -2., 0.), (4., 3., 6., 4.)] {
        let surface = NurbsSurface::try_bilinear([
            point(-3., y, z),
            point(3., y, z),
            point(4.2, far_y, far_z),
            point(-1.8, far_y, far_z),
        ])
        .unwrap();
        edges.push(add_profile_edge(&mut app, surface, |p| {
            p.distance_to(point(0., y, z)).unwrap() < 1e-9
        }));
    }
    app.commands
        .execute(&mut app.document, "Line -2,2,1.4 4,2,1.4")
        .unwrap();
    let middle = app.document.objects().last().unwrap().id();
    let sources = app.document.objects().cloned().collect::<Vec<_>>();
    app.document.clear_history().unwrap();
    enter(&mut app, "Loft");
    enter(&mut app, &edges[0]);
    enter(&mut app, &format!("SelID={middle}"));
    enter(&mut app, &edges[1]);
    enter(&mut app, "");
    let review = |app: &VibocerosApp| app.loft_prompt.as_ref().unwrap().preview_wires().to_vec();
    let baseline = review(&app);
    assert!(!baseline.is_empty());
    assert!(
        app.command_log
            .back()
            .unwrap()
            .contains("MatchStartTangent=No")
    );
    enter(&mut app, "MatchStartTangent");
    enter(&mut app, "Yes");
    let start_only = review(&app);
    assert!(!start_only.is_empty(), "{:?}", app.command_log);
    assert_ne!(start_only, baseline);
    enter(&mut app, "MatchEndTangent=Yes");
    let both = review(&app);
    assert!(!both.is_empty(), "{:?}", app.command_log);
    assert_ne!(start_only, both);
    enter(&mut app, "Undo");
    assert_eq!(review(&app), start_only);
    enter(&mut app, "MatchEndTangent=Yes");
    assert_eq!(review(&app), both);
    enter(&mut app, "Type=Loose");
    assert!(!review(&app).is_empty());
    assert!(
        !app.command_log
            .back()
            .unwrap()
            .contains("MatchStartTangent")
    );
    enter(&mut app, "Type=Normal");
    assert_eq!(review(&app), both);
    enter(&mut app, "Rebuild=6");
    assert!(!review(&app).is_empty(), "{:?}", app.command_log);
    assert!(
        app.command_log
            .back()
            .unwrap()
            .contains("MatchEndTangent=Yes")
    );
    enter(&mut app, "Refit=0.001");
    assert!(!review(&app).is_empty(), "{:?}", app.command_log);
    enter(&mut app, "Simplify=None");
    assert_eq!(review(&app), both);
    assert_eq!(app.document.objects().len(), 3);
    assert_eq!(app.document.undo_label(), None);
    enter(&mut app, "");
    assert!(app.loft_prompt.is_none(), "{:?}", app.command_log);
    assert_eq!(app.document.undo_label(), Some("Loft"));
    assert!(app.document.objects().take(3).eq(&sources));
    let Geometry::Brep(brep) = app.document.objects().last().unwrap().geometry() else {
        panic!("Loft B-rep");
    };
    let surface = brep.faces()[0].surface();
    let domain_u = surface.domain_u();
    let domain_v = surface.domain_v();
    let mid_v = domain_v.start().midpoint(*domain_v.end());
    // Both shifted planar profiles and the three-section chart were sampled
    // in Rhino 8.32 with the two tangent matching boxes checked.
    for (fraction, expected) in [
        (
            0.1,
            point(0.10606795359010111, 0.5723894524725227, 0.08978168623511162),
        ),
        (
            0.5,
            point(1.000017136840151, 2.030668268425833, 1.4389161131634247),
        ),
        (
            0.9,
            point(0.10209620617256243, 3.474157510988216, 2.713288811300691),
        ),
    ] {
        let u = *domain_u.start() + fraction * (domain_u.end() - domain_u.start());
        assert!(
            surface
                .evaluate(u, mid_v)
                .unwrap()
                .distance_to(expected)
                .unwrap()
                < 1e-9
        );
    }
    let middle_parameter = surface.knots_u()[4];
    assert!(
        surface
            .evaluate(middle_parameter, mid_v)
            .unwrap()
            .distance_to(point(1., 2., 1.4))
            .unwrap()
            < 1e-9
    );
}

#[test]
fn loft_tangency_on_cylindrical_rims_matches_the_native_preview_shape() {
    check_round_rim_loft(
        [[3., 3.], [2.5, 2.5]],
        None,
        [
            (0.1, 3.076328336831772, 0.6861039032687677),
            (0.5, 3.5850086468978044, 3.1116938319537555),
            (0.9, 2.5978532030669665, 5.353855973225749),
        ],
    );
}

#[test]
fn loft_tangency_on_conical_rims_matches_the_native_preview_shape() {
    check_round_rim_loft(
        [[2.3, 3.], [2.5, 3.4]],
        None,
        [
            (0.1, 3.243158846134268, 0.6572593641100615),
            (0.5, 3.5592321507563667, 3.1167230910321257),
            (0.9, 2.3964509218026087, 5.3966638741522965),
        ],
    );
}

#[test]
fn loft_tangency_on_rebuilt_rims_follows_the_original_supports() {
    check_round_rim_loft(
        [[2.3, 3.], [2.5, 3.4]],
        Some(10),
        [
            (0.1, 3.2410618764369246, 0.6461814365456595),
            (0.5, 3.573403368789612, 3.0759669366336073),
            (0.9, 2.3961935200660816, 5.386621457804872),
        ],
    );
}

fn check_round_rim_loft(
    radii: [[f64; 2]; 2],
    rebuild: Option<usize>,
    samples: [(f64, f64, f64); 3],
) {
    let mut app = test_app();
    let plane = app.viewports[0].construction_plane();
    let tolerance = app.document.tolerance();
    let circle = |radius, z| {
        Circle3::try_new(point(0., 0., z), radius, plane.z_axis(), tolerance)
            .unwrap()
            .to_nurbs()
            .unwrap()
    };
    let mut rims = Vec::new();
    for ([r0, r1], (from, to, rim_z)) in radii.into_iter().zip([(-2., 0., 0.), (6., 8., 6.)]) {
        let surface = if r0 == r1 {
            NurbsSurface::try_cylinder(plane, r0, from, to).unwrap()
        } else {
            NurbsSurface::try_ruled_from_curves(&circle(r0, from), &circle(r1, to)).unwrap()
        };
        rims.push(add_profile_edge(&mut app, surface, |p| {
            (p.z() - rim_z).abs() < 1e-9
        }));
    }
    app.commands
        .execute(&mut app.document, "Circle 0,0,3 3.6")
        .unwrap();
    let middle = app.document.objects().last().unwrap().id();
    let sources = app.document.objects().cloned().collect::<Vec<_>>();
    app.document.clear_history().unwrap();
    enter(&mut app, "Loft");
    enter(&mut app, &rims[0]);
    enter(&mut app, &format!("SelID={middle}"));
    enter(&mut app, &rims[1]);
    enter(&mut app, "");
    enter(&mut app, "");
    enter(&mut app, "MatchStartTangent=Yes MatchEndTangent=Yes");
    let review = |app: &VibocerosApp| app.loft_prompt.as_ref().unwrap().preview_wires().to_vec();
    let matched = review(&app);
    assert!(!matched.is_empty(), "{:?}", app.command_log);
    if let Some(count) = rebuild {
        enter(&mut app, &format!("Rebuild={count}"));
        let rebuilt = review(&app);
        assert!(!rebuilt.is_empty(), "{:?}", app.command_log);
        assert_ne!(rebuilt, matched);
        enter(&mut app, "Undo");
        assert_eq!(review(&app), matched);
        enter(&mut app, &format!("Rebuild={count}"));
        assert_eq!(review(&app), rebuilt);
    }
    assert_eq!(app.document.undo_label(), None);
    enter(&mut app, "");
    assert!(app.loft_prompt.is_none(), "{:?}", app.command_log);
    assert_eq!(app.document.undo_label(), Some("Loft"));
    assert!(app.document.objects().take(3).eq(&sources));
    let Geometry::Brep(brep) = app.document.objects().last().unwrap().geometry() else {
        panic!("Loft B-rep");
    };
    let surface = brep.faces()[0].surface();
    if let Some(count) = rebuild {
        assert_eq!(surface.control_point_count_v(), count + 1);
        // Rebuild interpolates the edge at the fit stations. The new boundary
        // can leave the original rim between those stations.
        let [u, v] = [surface.domain_u(), surface.domain_v()];
        let departure = |u: f64, radius: f64| {
            (0..=32).fold(0.0_f64, |max, i| {
                let station = *v.start() + (v.end() - v.start()) * f64::from(i) / 32.;
                let point = surface.evaluate(u, station).unwrap();
                max.max((point.x().hypot(point.y()) - radius).abs())
            })
        };
        let start_height = surface.evaluate(*u.start(), *v.start()).unwrap().z();
        let (start, end) = if start_height > 3. {
            (*u.end(), *u.start())
        } else {
            (*u.start(), *u.end())
        };
        assert!((departure(start, 3.) - 0.0040065320630437995).abs() < 1e-7);
        assert!((departure(end, 2.5) - 0.0033387767192034623).abs() < 1e-7);
    }
    let domain_u = surface.domain_u();
    let domain_v = surface.domain_v();
    let mid_v = domain_v.start().midpoint(*domain_v.end());
    // Rim orientation can reverse the loft chart. Measure from the lower rim.
    let reversed = surface.evaluate(*domain_u.start(), mid_v).unwrap().z() > 3.;
    for (fraction, radius, height) in samples {
        let fraction = if reversed { 1. - fraction } else { fraction };
        let u = *domain_u.start() + fraction * (domain_u.end() - domain_u.start());
        let sample = surface.evaluate(u, mid_v).unwrap();
        assert!(
            (sample.x().hypot(sample.y()) - radius).abs() < 1e-9,
            "{sample:?}"
        );
        assert!((sample.z() - height).abs() < 1e-9, "{sample:?}");
    }
}
