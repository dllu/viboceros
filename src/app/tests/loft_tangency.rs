use super::*;
use viboceros_geometry::{Brep, Circle3, Ellipse3, NurbsSurface};

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

fn review_closed_profiles(app: &mut VibocerosApp, edges: [&str; 2], middle: ObjectId) {
    enter(app, "Loft");
    enter(app, edges[0]);
    enter(app, &format!("SelID={middle}"));
    enter(app, edges[1]);
    enter(app, "");
    enter(app, "");
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

#[test]
fn loft_tangency_on_skewed_elliptical_rims_retains_native_patch_shapes() {
    let mut app = test_app();
    let plane = app.viewports[0].construction_plane();
    let tolerance = app.document.tolerance();
    let ellipse = |[x, y, z, rx, ry]: [f64; 5]| {
        Ellipse3::try_new(
            point(x, y, z),
            rx,
            ry,
            plane.x_axis(),
            plane.y_axis(),
            tolerance,
        )
        .unwrap()
        .to_nurbs()
        .unwrap()
    };
    let [start, end] = [
        ([0., 0., 0., 3., 1.8], [-0.8, 0.7, -2., 2.4, 1.4]),
        ([0.3, -0.6, 6., 2.5, 1.75], [0.9, -0.4, 8., 3.4, 2.6]),
    ]
    .map(|(near, far)| {
        let surface = NurbsSurface::try_ruled_from_curves(&ellipse(near), &ellipse(far)).unwrap();
        add_profile_edge(&mut app, surface, |p| (p.z() - near[2]).abs() < 1e-9)
    });
    let middle = app
        .document
        .add_geometry(Geometry::NurbsCurve(ellipse([0.4, 0.3, 3., 3.6, 1.8])))
        .unwrap();
    app.document.clear_history().unwrap();
    // Rhino 8.32 samples from all four faces, away from the shared edges.
    let both_ends = [
        [2.929295134737457, -0.9615899476662249, 3.086400396121126],
        [1.4055001945747467, -1.5724889733774483, 4.523185162468901],
        [-2.125070805930426, -0.9754008211782568, 3.084618274145645],
        [
            -0.47046258332083873,
            -1.8163336109078367,
            1.5439022398047277,
        ],
        [-2.129835061714781, 1.5576065392790062, 3.0848709515470176],
        [-3.07333913468666, 0.5033074838913203, 1.6042498218578982],
        [2.933764474877405, 1.543189121890091, 3.082885446564188],
        [2.8221397487763347, 0.25764680097466025, 4.511704386295492],
    ];
    for (edges, options, first_height) in [
        (
            [start.as_str(), end.as_str()],
            "MatchStartTangent=Yes MatchEndTangent=Yes",
            0.,
        ),
        ([end.as_str(), start.as_str()], "MatchStartTangent=Yes", 6.),
    ] {
        review_closed_profiles(&mut app, edges, middle);
        enter(&mut app, options);
        assert!(
            !app.loft_prompt.as_ref().unwrap().preview_wires().is_empty(),
            "{:?}",
            app.command_log
        );
        assert_eq!(app.document.undo_label(), None);
        enter(&mut app, "");
        assert!(app.loft_prompt.is_none(), "{:?}", app.command_log);
        assert_eq!(app.document.undo_label(), Some("Loft"));
        let Geometry::Brep(brep) = app.document.objects().last().unwrap().geometry() else {
            panic!("Loft B-rep");
        };
        assert_eq!(brep.faces().len(), 4);
        let surface = brep.faces()[0].surface();
        let first = surface
            .evaluate(*surface.domain_u().start(), *surface.domain_v().start())
            .unwrap();
        assert!((first.z() - first_height).abs() < 1e-9);
        if first_height == 0. {
            for [x, y, z] in both_ends {
                let expected = point(x, y, z);
                let (face, u, v) = brep
                    .closest_face_parameters(expected, tolerance)
                    .unwrap()
                    .unwrap();
                let actual = brep.faces()[face].surface().evaluate(u, v).unwrap();
                assert!(
                    actual.distance_to(expected).unwrap() < 1e-9,
                    "{actual:?} != {expected:?}"
                );
            }
        } else {
            check_reversed_start_handles(brep);
        }
        enter(&mut app, "Undo");
        assert_eq!(app.document.objects().len(), 3);
    }
}

fn check_reversed_start_handles(brep: &Brep) {
    // Native first-row controls with the upper rim selected first and only
    // MatchStartTangent enabled. The opposite row follows a different policy.
    for ([x, y, z], [hx, hy, hz]) in [
        (
            [2.8, -0.6, 6.],
            [1.9907234774930678, -0.6, 4.920964636657423],
        ),
        (
            [2.8, 1.15, 6.],
            [2.394375839707634, 0.11926795447648368, 4.962582545570265],
        ),
        (
            [0.3, 1.15, 6.],
            [0.3, 0.6320813647824985, 5.0134883138714255],
        ),
        (
            [-2.2, 1.15, 6.],
            [-1.8145076474329924, 0.7824475617872548, 4.90628851826779],
        ),
        (
            [-2.2, -0.6, 6.],
            [-2.018767118436487, -0.6, 4.791780789576578],
        ),
        (
            [-2.2, -2.35, 6.],
            [-2.056250011517065, -2.199998135145472, 4.8740789240494795],
        ),
        (
            [0.3, -2.35, 6.],
            [0.3, -2.0128117218484487, 4.962497605687534],
        ),
        (
            [2.8, -2.35, 6.],
            [2.5828196074377545, -1.4606776759004867, 4.843125537276538],
        ),
    ] {
        let boundary = point(x, y, z);
        let handle = brep
            .faces()
            .iter()
            .find_map(|face| {
                let surface = face.surface();
                let column = (0..surface.control_point_count_v()).find(|&v| {
                    surface
                        .control_point(0, v)
                        .unwrap()
                        .point()
                        .distance_to(boundary)
                        .unwrap()
                        < 1e-9
                })?;
                surface
                    .control_point(1, column)
                    .map(|control| control.point())
            })
            .expect("matched boundary control");
        let expected = point(hx, hy, hz);
        assert!(
            handle.distance_to(expected).unwrap() < 1e-9,
            "{handle:?} != {expected:?}"
        );
    }
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
    let [start, end] =
        [(radii[0], -2., 0., 0.), (radii[1], 6., 8., 6.)].map(|([r0, r1], from, to, rim_z)| {
            let surface = if r0 == r1 {
                NurbsSurface::try_cylinder(plane, r0, from, to).unwrap()
            } else {
                NurbsSurface::try_ruled_from_curves(&circle(r0, from), &circle(r1, to)).unwrap()
            };
            add_profile_edge(&mut app, surface, |p| (p.z() - rim_z).abs() < 1e-9)
        });
    app.commands
        .execute(&mut app.document, "Circle 0,0,3 3.6")
        .unwrap();
    let middle = app.document.objects().last().unwrap().id();
    let sources = app.document.objects().cloned().collect::<Vec<_>>();
    app.document.clear_history().unwrap();
    review_closed_profiles(&mut app, [&start, &end], middle);
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
        assert!((departure(*u.start(), 3.) - 0.0040065320630437995).abs() < 1e-7);
        assert!((departure(*u.end(), 2.5) - 0.0033387767192034623).abs() < 1e-7);
    }
    let domain_u = surface.domain_u();
    let domain_v = surface.domain_v();
    let mid_v = domain_v.start().midpoint(*domain_v.end());
    assert!(
        surface
            .evaluate(*domain_u.start(), mid_v)
            .unwrap()
            .z()
            .abs()
            < 1e-9
    );
    for (fraction, radius, height) in samples {
        let u = *domain_u.start() + fraction * (domain_u.end() - domain_u.start());
        let sample = surface.evaluate(u, mid_v).unwrap();
        assert!(
            (sample.x().hypot(sample.y()) - radius).abs() < 1e-9,
            "{sample:?}"
        );
        assert!((sample.z() - height).abs() < 1e-9, "{sample:?}");
    }
}
