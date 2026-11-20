use super::*;

fn p(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
fn sources(d: &mut Document) -> Vec<ObjectId> {
    [
        (p(0., 0., 0.), p(0., 0., 5.)),
        (p(2., 0., 5.), p(2., 0., 0.)),
        (p(2., 0., 5.), p(0., 0., 5.)),
        (p(0., 0., 0.), p(2., 0., 0.)),
    ]
    .into_iter()
    .map(|(a, b)| {
        d.add_geometry(Geometry::Line(
            LineSegment::try_new(a, b, d.tolerance()).unwrap(),
        ))
        .unwrap()
    })
    .collect()
}
#[test]
fn sweep_two_rail_arguments_and_selection_failures_preserve_document_and_redo() {
    let r = CommandRegistry::with_builtins();
    let mut d = Document::default();
    let ids = sources(&mut d);
    d.select_objects_direct(ids.clone(), SelectionMode::Replace)
        .unwrap();
    d.add_geometry(Geometry::Point(p(8., 8., 8.))).unwrap();
    d.undo().unwrap();
    let before = format!("{d:?}");
    for args in [
        "Sweep2 Bad=Yes",
        "Sweep2 Rail1Name=Rail",
        "Sweep2 Rail1Name=A Rail1Name=B Rail2Name=C",
        "Sweep2 Rail1Name= Rail2Name=B",
        "Sweep2 Rail1Name=Missing Rail2Name=Missing",
        "Sweep2 Rebuild=129",
        "Sweep2 Refit=NaN",
        "Sweep2 Rebuild=5 Refit=0.01",
        "Sweep2 MaintainHeight=Maybe",
        "Sweep2 MaintainHeight=Yes MaintainHeight=No",
    ] {
        assert!(r.execute(&mut d, args).is_err(), "{args}");
        assert_eq!(format!("{d:?}"), before);
        assert!(d.can_redo());
    }
    let prompt = r.object_selection_prompt("Sweep2").unwrap().unwrap();
    assert_eq!(prompt.filter, ObjectSelectionFilter::Curves);
    assert_eq!(
        prompt.workflow,
        ObjectSelectionWorkflow::OptionsDuringSelection
    );
    assert!(r.command_names().contains(&"Sweep2"));
}

#[test]
fn sweep_two_rail_height_matches_native_tapered_arches() {
    let registry = CommandRegistry::with_builtins();
    // Rhino 8.32 -Sweep2 captures. The middle profile alone retains the
    // second half of the rails. All of these output tensors have one face.
    for (profiles, maintain, area, heights) in [
        (&[(0., 0.8)][..], false, 26.937939236782057, [0.6, 0.8, 1.]),
        (&[(0., 0.8)][..], true, 24.793555945765071, [0.4; 3]),
        (
            &[(0.5, 0.8)][..],
            false,
            15.492558738252047,
            [0.45, 0.5, 0.55],
        ),
        (&[(0.5, 0.8)][..], true, 15.299734691756731, [0.4; 3]),
        (
            &[(0., 0.8), (1., 1.2)][..],
            true,
            25.177959211423175,
            [0.43125, 0.5, 0.56875],
        ),
        (
            &[(0., 0.8), (0.5, 2.), (1., 1.)][..],
            false,
            26.808254702885652,
            [0.675, 1., 5. / 6.],
        ),
        (
            &[(0., 0.8), (0.5, 2.), (1., 1.)][..],
            true,
            26.691353542756822,
            [0.7, 1., 0.75],
        ),
    ] {
        let mut doc = Document::default();
        let tolerance = doc.tolerance();
        let mut ids = [
            (p(0., 0., 0.), p(0., 0., 6.)),
            (p(6., 0., 6.), p(2., 0., 0.)),
        ]
        .into_iter()
        .map(|(a, b)| {
            doc.add_geometry(Geometry::Line(
                LineSegment::try_new(a, b, tolerance).unwrap(),
            ))
            .unwrap()
        })
        .collect::<Vec<_>>();
        for &(t, height) in profiles {
            let z = 6. * t;
            let width = 2. + 4. * t;
            let arch = NurbsCurve::try_new(
                2,
                vec![p(0., 0., z), p(width / 2., height, z), p(width, 0., z)],
                vec![0., 0., 0., 1., 1., 1.],
            )
            .unwrap();
            ids.push(doc.add_geometry(Geometry::NurbsCurve(arch)).unwrap());
        }
        doc.select_objects_direct(ids, SelectionMode::Replace)
            .unwrap();
        let height = if maintain { "_Yes" } else { "_No" };
        registry
            .execute(&mut doc, &format!("Sweep2 MaintainHeight={height}"))
            .unwrap();
        let Geometry::Brep(brep) = doc.objects().last().unwrap().geometry() else {
            panic!("Expected a Sweep2 B-rep");
        };
        assert_eq!(brep.faces().len(), 1);
        let surface = brep.faces()[0].surface();
        let start = profiles[0].0;
        for (u, height) in [0.25, 0.5, 0.75].into_iter().zip(heights) {
            let station = start + u * (1. - start);
            for v in [0.25, 0.5, 0.75] {
                let expected = p(
                    (2. + 4. * station) * v,
                    4. * v * (1. - v) * height,
                    6. * station,
                );
                let actual = surface.evaluate(u, v).unwrap();
                assert!(
                    actual.distance_to(expected).unwrap() < 1e-8,
                    "{actual:?} != {expected:?}"
                );
            }
        }
        let actual_area = brep.area(tolerance).unwrap();
        assert!((actual_area - area).abs() < 1e-5, "{actual_area} != {area}");
    }
}

#[test]
fn sweep_two_rail_ordered_and_named_inputs_create_one_undoable_brep() {
    for named in [false, true] {
        let r = CommandRegistry::with_builtins();
        let mut d = Document::default();
        let ids = sources(&mut d);
        d.set_object_names([
            (ids[0], Some("Left".into())),
            (ids[1], Some("Right".into())),
        ])
        .unwrap();
        d.select_objects_direct(
            if named {
                ids[2..].to_vec()
            } else {
                ids.clone()
            },
            SelectionMode::Replace,
        )
        .unwrap();
        let before = d.objects().cloned().collect::<Vec<_>>();
        let selection = d.selected_object_ids().collect::<Vec<_>>();
        let output = r
            .execute(
                &mut d,
                if named {
                    "Sweep2 Rail1Name=left Rail2Name=RIGHT"
                } else {
                    "Sweep2"
                },
            )
            .unwrap();
        assert!(output.contains("2 sections"));
        assert_eq!(d.objects().count(), 5);
        assert_eq!(d.undo_label(), Some("Sweep2"));
        for source in &before {
            assert_eq!(d.object(source.id()), Some(source));
        }
        assert_eq!(d.selected_object_ids().collect::<Vec<_>>(), selection);
        let result = d
            .objects()
            .find(|o| !ids.contains(&o.id()))
            .unwrap()
            .clone();
        let Geometry::Brep(b) = result.geometry() else {
            panic!("expected a B-rep")
        };
        assert_eq!(b.faces().len(), 1);
        assert!((b.area(d.tolerance()).unwrap() - 10.).abs() < 1e-8);
        assert_eq!(
            crate::geometry_check::check_geometry(result.geometry()).status,
            crate::geometry_check::GeometryCheckStatus::Valid
        );
        r.execute(&mut d, "Undo").unwrap();
        assert_eq!(d.objects().cloned().collect::<Vec<_>>(), before);
        r.execute(&mut d, "Redo").unwrap();
        assert_eq!(d.object(result.id()), Some(&result));
    }
}

#[test]
fn sweep_two_rail_section_fits_match_the_native_dialog() {
    let mut doc = Document::default();
    let mut ids = vec![];
    for x in [0., 2.] {
        let line = LineSegment::try_new(p(x, 0., 0.), p(x, 0., 5.), doc.tolerance()).unwrap();
        ids.push(doc.add_geometry(Geometry::Line(line)).unwrap());
    }
    for z in [0., 5.] {
        let arch = NurbsCurve::try_new(
            2,
            vec![p(0., 0., z), p(1., 0.3, z + 0.7), p(2., 0., z)],
            vec![0., 0., 0., 1., 1., 1.],
        )
        .unwrap();
        ids.push(doc.add_geometry(Geometry::NurbsCurve(arch)).unwrap());
    }
    doc.select_objects_direct(ids, SelectionMode::Replace)
        .unwrap();
    let rebuilt = [
        [0., 0., 0.],
        [
            0.28199862089929856,
            0.08689181718005236,
            0.20274757342012226,
        ],
        [1.0000000054599303, 0.21310818064132253, 0.49725242149641924],
        [1.7180013564163852, 0.08689182153730256, 0.202747583587038],
        [2., 0., 0.],
    ];
    let refitted = [
        [0., 0., 0.],
        [0.6666667013482039, 0.20000001040446116, 0.4666666909437428],
        [1.333333298651796, 0.20000001040446116, 0.4666666909437428],
        [2., 0., 0.],
    ];
    let linear = [[0., 0., 0.], [2., 0., 0.]];
    // Rhino 8.32 interactive Sweep2 with Rebuild = 2 or 5 and Refit = 0.01.
    // Equal normalized polynomial bases make the control bounds valid
    // throughout the entire tensor.
    let registry = CommandRegistry::with_builtins();
    for (option, expected, degree, midpoint) in [
        ("Rebuild=2", linear.as_slice(), 1, None),
        ("Rebuild=5", rebuilt.as_slice(), 3, Some(0.5)),
        ("Refit=0.01", refitted.as_slice(), 3, None),
    ] {
        registry
            .execute(&mut doc, &format!("Sweep2 {option}"))
            .unwrap();
        let Geometry::Brep(brep) = doc.objects().last().unwrap().geometry() else {
            panic!("Expected a Sweep2 B-rep");
        };
        assert_eq!(brep.faces().len(), 1);
        let surface = brep.faces()[0].surface();
        assert_eq!([surface.degree_u(), surface.degree_v()], [1, degree]);
        assert_eq!(
            [
                surface.control_point_count_u(),
                surface.control_point_count_v()
            ],
            [2, expected.len()],
        );
        let domain = surface.domain_v();
        let normalized = surface
            .knots_v()
            .iter()
            .map(|v| (v - domain.start()) / (domain.end() - domain.start()))
            .collect::<Vec<_>>();
        let knots = std::iter::repeat_n(0., degree + 1)
            .chain(midpoint)
            .chain(std::iter::repeat_n(1., degree + 1))
            .collect::<Vec<_>>();
        assert_eq!(normalized, knots);
        for (u, z) in [0., 5.].into_iter().enumerate() {
            for (v, [x, y, height]) in expected.iter().copied().enumerate() {
                let control = surface.control_point(u, v).unwrap();
                assert_eq!(control.weight(), 1.);
                let error = control.point().distance_to(p(x, y, height + z)).unwrap();
                assert!(
                    error < 1e-6,
                    "{option} control [{u}, {v}] differed by {error}"
                );
            }
        }
        registry.execute(&mut doc, "Undo").unwrap();
    }
}
