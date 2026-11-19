use super::*;

#[test]
fn oblique_boundary_stitches_a_shallow_clip_to_the_selected_cap() {
    let tolerance = Tolerance::try_new(1e-6, 1e-12, 1e-8).unwrap();
    for (lower, edges, volume, surface) in [
        (-2.287620967290623, 14, Some(110.25562389137261), false),
        (-2.287602897246215, 13, Some(110.25562353867707), false),
        (-2.287620967290623, 14, None, false),
        (-2.287620967290623, 14, Some(110.25562233170592), true),
    ] {
        let support = cutter([-2., lower, 3.7, 2.7], false);
        let (mut doc, source, reference) = scene(tolerance, &support, surface);
        let commands = CommandRegistry::with_builtins();
        if surface {
            commands.execute(&mut doc, &format!("ExtrudeSrfTapered Direction=1,0,6 DraftAngle=8 ToBoundary={reference} Solid=No DeleteInput=Yes")).unwrap();
        } else {
            let solid = if volume.is_some() {
                "Solid=Yes"
            } else {
                "Solid=No"
            };
            let options = ["Direction=1,0,6", "DraftAngle=8", solid, "DeleteInput=Yes"];
            assert_eq!(
                pick(&mut doc, &commands, reference, &support, &options).unwrap(),
                TaperedBoundaryOutcome::Cut(1),
                "{lower}"
            );
        }
        assert!(doc.object(source).is_none());
        let Geometry::Brep(result) = doc
            .objects()
            .find(|o| o.id() != reference.object)
            .unwrap()
            .geometry()
        else {
            panic!("expected the stitched sweep");
        };
        assert_eq!(result.is_solid(), volume.is_some());
        assert_eq!(result.edges().len(), edges);
        if let Some(volume) = volume {
            assert_eq!(result.faces().len(), 6);
            let cap = result.faces().last().unwrap();
            assert!(
                (cap.trim_boundary_bounds(tolerance).unwrap().min().y() - lower).abs()
                    < tolerance.absolute()
            );
            assert!((result.signed_volume(tolerance).unwrap() - volume).abs() < 1e-4);
        } else {
            assert_eq!(result.faces().len(), 4);
        }
        let repaired = result
            .edges()
            .iter()
            .filter(|edge| edge.tolerance() > 1e-5)
            .collect::<Vec<_>>();
        let [edge] = repaired.as_slice() else {
            panic!("expected one stitched edge")
        };
        for control in edge.curve().control_points() {
            assert!((control.point().y() - lower).abs() < tolerance.absolute());
        }
        assert!(edge.tolerance() <= 1e-4);
        if surface {
            let mesh = result.tessellate(3, tolerance).unwrap();
            assert!(mesh.topology().is_closed());
        }
        commands.execute(&mut doc, "Undo").unwrap();
        assert!(doc.is_selected(source));
        commands.execute(&mut doc, "Redo").unwrap();
        assert!(doc.object(source).is_none());
    }
}

#[test]
fn oblique_boundary_stitches_only_within_the_captured_join_gap() {
    let tolerance = Tolerance::try_new(1e-6, 1e-12, 1e-8).unwrap();
    let cosine = 3_f64.sqrt() / 2.;
    for (warped, depth, accepted) in [
        (false, 9.5e-5, true),
        (false, 1e-4, true),
        (false, 1.02e-4, false),
        (true, 2.5e-5, true),
        (true, 1.1e-4, false),
    ] {
        let rim_minimum = if warped {
            -2.704941626075615
        } else {
            -2.7102038548174896
        };
        let bottom = rim_minimum + depth;
        let corner = |x, y| {
            let u = (x + 9.) / 24.;
            let v = (y + 9.) / 22.;
            let z = if warped {
                4.3 + 1.2 * u + 0.9 * v + 0.4 * u * v
            } else {
                5.
            };
            point(cosine * x - 0.5 * y, 0.5 * x + cosine * y, z)
        };
        let support = NurbsSurface::try_bilinear([
            corner(-4., bottom),
            corner(5., bottom),
            corner(5., 4.),
            corner(-4., 4.),
        ])
        .unwrap();
        let (mut doc, source, reference) = scene(tolerance, &support, false);
        let commands = CommandRegistry::with_builtins();
        let outcome = pick(
            &mut doc,
            &commands,
            reference,
            &support,
            &[
                "Direction=1,0,6",
                "DraftAngle=8",
                "Solid=Yes",
                "DeleteInput=Yes",
            ],
        )
        .unwrap();
        assert_eq!(
            outcome,
            if accepted {
                TaperedBoundaryOutcome::Cut(1)
            } else {
                TaperedBoundaryOutcome::Fallback
            },
            "warped={warped}, depth={depth}"
        );
        assert!(doc.object(source).is_none());
        let Geometry::Brep(result) = doc
            .objects()
            .find(|o| o.id() != reference.object)
            .unwrap()
            .geometry()
        else {
            panic!("expected a sweep");
        };
        assert_eq!(result.is_solid(), accepted);
        assert_eq!(result.edges().len(), if accepted { 14 } else { 12 });
        if warped && accepted {
            let volume = result.signed_volume(tolerance).unwrap();
            assert!((volume - 113.50101264808393).abs() < 1e-4, "{volume}");
            let cap = result.faces().last().unwrap();
            let bounds = cap.trim_boundary_bounds(tolerance).unwrap();
            let expected = point(3.1341718737870865, 2.236292349034467, 5.370308609255279);
            assert!(
                bounds.max().distance_to(expected).unwrap() < 2. * tolerance.absolute(),
                "{bounds:?}"
            );
        }
    }
}
