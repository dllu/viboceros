use super::*;

#[test]
fn toggle_follows_option_order_and_remembers_only_accepted_choices() {
    let r = CommandRegistry::with_builtins();
    let (mut d, _) = selected(Geometry::NurbsSurface(surface()));
    for command in [
        "ConvertToSingleSpans Toggle",
        "ConvertToSingleSpans Direction=Both Toggle",
        "ConvertToSingleSpans Direction=U Toggle Other=Yes",
    ] {
        assert!(r.execute(&mut d, command).is_err());
    }
    check_preference(&r, SurfaceKnotDirection::Both, false);
    r.execute(
        &mut d,
        "ConvertToSingleSpans Direction=U DeleteInput=Yes _Toggle",
    )
    .unwrap();
    check_preference(&r, SurfaceKnotDirection::V, true);
    let (mut d, _) = selected(Geometry::NurbsSurface(surface()));
    r.execute(
        &mut d,
        "ConvertToSingleSpans Toggle Toggle Toggle DeleteInput=No",
    )
    .unwrap();
    check_preference(&r, SurfaceKnotDirection::U, false);
}

#[test]
fn remembering_a_noop_does_not_clear_redo_or_add_a_history_entry() {
    let (mut d, multi) = selected(Geometry::NurbsSurface(surface()));
    let single = d
        .add_geometry(Geometry::NurbsSurface(
            surface().try_bezier_patches().unwrap().remove(0),
        ))
        .unwrap();
    let r = CommandRegistry::with_builtins();
    r.execute(&mut d, "ConvertToSingleSpans Direction=U DeleteInput=Yes")
        .unwrap();
    r.execute(&mut d, "Undo").unwrap();
    assert!(d.can_redo());
    d.select_objects_direct([single], SelectionMode::Replace)
        .unwrap();
    let history = d.undo_label().map(str::to_owned);
    r.execute(&mut d, "ConvertToSingleSpans Direction=V DeleteInput=No")
        .unwrap();
    assert_eq!(d.undo_label(), history.as_deref());
    assert!(d.can_redo());
    r.execute(&mut d, "Redo").unwrap();
    assert!(d.object(multi).is_none());
    check_preference(&r, SurfaceKnotDirection::V, false);
}

#[test]
fn unrepresentable_later_source_rolls_back_everything_and_does_not_accept_options() {
    let r = CommandRegistry::with_builtins();
    let (mut seed, _) = selected(Geometry::NurbsSurface(surface()));
    r.execute(
        &mut seed,
        "ConvertToSingleSpans Direction=U DeleteInput=Yes",
    )
    .unwrap();
    let (mut d, first) = selected(Geometry::NurbsSurface(surface()));
    let bad = NurbsSurface::try_new_rational(
        2,
        1,
        4,
        2,
        (0..8)
            .map(|i| {
                WeightedPoint3::try_new(
                    Point3::try_new((i % 4) as f64, (i / 4) as f64, 0.).unwrap(),
                    [1., -1., 3., 4.][i % 4],
                )
                .unwrap()
            })
            .collect(),
        vec![-2., -1., 0., 1., 2., 3., 4.],
        vec![0., 0., 1., 1.],
    )
    .unwrap();
    let second = d.add_geometry(Geometry::NurbsSurface(bad)).unwrap();
    d.select_objects_direct([second], SelectionMode::Add)
        .unwrap();
    let group = d
        .add_group(Some("sources".into()), [first, second])
        .unwrap();
    let before = d.objects().cloned().collect::<Vec<_>>();
    let history = d.undo_label().map(str::to_owned);
    assert!(matches!(
        r.execute(&mut d, "ConvertToSingleSpans Direction=Both DeleteInput=No"),
        Err(CommandError::Geometry(
            GeometryError::UnrepresentableBezierControl
        ))
    ));
    assert_eq!(d.objects().cloned().collect::<Vec<_>>(), before);
    assert_eq!(d.selected_object_ids().collect::<Vec<_>>(), [first, second]);
    assert_eq!(
        d.group(group).unwrap().members().collect::<BTreeSet<_>>(),
        BTreeSet::from([first, second])
    );
    assert_eq!(d.undo_label(), history.as_deref());
    check_preference(&r, SurfaceKnotDirection::U, true);
}

fn surface() -> NurbsSurface {
    NurbsSurface::try_new_rational(
        2,
        1,
        4,
        3,
        (0..12)
            .map(|i| {
                WeightedPoint3::try_new(
                    Point3::try_new(
                        (i % 4) as f64,
                        (i / 4) as f64,
                        if i % 4 == 1 { 2. } else { 0. },
                    )
                    .unwrap(),
                    [1., 0.6, 1.4, 1.][i % 4],
                )
                .unwrap()
            })
            .collect(),
        vec![-2., -2., -2., 1., 4., 4., 4.],
        vec![10., 10., 13., 18., 18.],
    )
    .unwrap()
}
fn selected(geometry: Geometry) -> (Document, ObjectId) {
    let mut d = Document::default();
    let id = d.add_geometry(geometry).unwrap();
    d.select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    (d, id)
}
fn check_preference(registry: &CommandRegistry, direction: SurfaceKnotDirection, delete: bool) {
    let (mut d, id) = selected(Geometry::NurbsSurface(surface()));
    registry.execute(&mut d, "ConvertToSingleSpans").unwrap();
    assert_eq!(d.object(id).is_none(), delete);
    assert_eq!(
        d.objects().len(),
        if direction == SurfaceKnotDirection::Both {
            4
        } else {
            2
        } + usize::from(!delete)
    );
    for o in d.objects().filter(|o| o.id() != id) {
        let Geometry::NurbsSurface(s) = o.geometry() else {
            panic!()
        };
        assert_eq!(
            s.domain_u(),
            if direction == SurfaceKnotDirection::V {
                -2.0..=4.0
            } else {
                0.0..=1.0
            }
        );
        assert_eq!(
            s.domain_v(),
            if direction == SurfaceKnotDirection::U {
                10.0..=18.0
            } else {
                0.0..=1.0
            }
        );
    }
}

#[test]
fn exact_strips_are_fresh_unselected_current_layer_objects_with_unit_split_domains() {
    for direction in ["U", "V", "Both"] {
        for delete in [false, true] {
            let s = surface();
            let (mut doc, source) = selected(Geometry::NurbsSurface(s.clone()));
            let registry = CommandRegistry::with_builtins();
            let group = doc.add_group(Some("Sources".into()), [source]).unwrap();
            let layer = doc.add_layer("Current", ColorRgb::BLACK).unwrap();
            doc.set_current_layer(layer).unwrap();
            let before = doc.objects().cloned().collect::<Vec<_>>();
            registry
                .execute(
                    &mut doc,
                    &format!(
                        "ConvertToSingleSpans Direction={direction} DeleteInput={}",
                        if delete { "Yes" } else { "No" }
                    ),
                )
                .unwrap();
            let u = direction != "V";
            let v = direction != "U";
            let count = if direction == "Both" { 4 } else { 2 };
            assert_eq!(doc.objects().len(), count + usize::from(!delete));
            assert_eq!(doc.object(source).is_some(), !delete);
            assert_eq!(
                doc.group(group).unwrap().members().len(),
                usize::from(!delete)
            );
            let du = if u {
                s.spans_u().collect::<Vec<_>>()
            } else {
                vec![(-2., 4.)]
            };
            let dv = if v {
                s.spans_v().collect::<Vec<_>>()
            } else {
                vec![(10., 18.)]
            };
            let domains = du.iter().flat_map(|a| dv.iter().map(move |b| (*a, *b)));
            for (object, ((a, b), (c, d))) in
                doc.objects().filter(|o| o.id() != source).zip(domains)
            {
                assert_eq!(object.attributes(), &ObjectAttributes::on_layer(layer));
                assert!(object.group_ids().is_empty());
                assert!(!doc.is_selected(object.id()));
                let Geometry::NurbsSurface(p) = object.geometry() else {
                    panic!()
                };
                assert_eq!(p.domain_u(), if u { 0.0..=1.0 } else { a..=b });
                assert_eq!(p.domain_v(), if v { 0.0..=1.0 } else { c..=d });
                for i in 0..=8 {
                    for j in 0..=8 {
                        let x = i as f64 / 8.;
                        let y = j as f64 / 8.;
                        assert!(
                            p.evaluate(
                                if u { x } else { a + (b - a) * x },
                                if v { y } else { c + (d - c) * y }
                            )
                            .unwrap()
                            .distance_to(s.evaluate(a + (b - a) * x, c + (d - c) * y).unwrap())
                            .unwrap()
                                < 2e-12
                        );
                    }
                }
            }
            let after = doc.objects().cloned().collect::<Vec<_>>();
            registry.execute(&mut doc, "Undo").unwrap();
            assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
            registry.execute(&mut doc, "Redo").unwrap();
            assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), after);
        }
    }
}

#[test]
fn aliases_share_preferences_across_documents_and_undo_does_not_restore_them() {
    let r = CommandRegistry::with_builtins();
    let (mut d, _) = selected(Geometry::NurbsSurface(surface()));
    r.execute(
        &mut d,
        "_ConvertSurfaceToSingleSpans Direction _U DeleteInput _Yes",
    )
    .unwrap();
    r.execute(&mut d, "Undo").unwrap();
    check_preference(&r, SurfaceKnotDirection::U, true);
    check_preference(
        &CommandRegistry::with_builtins(),
        SurfaceKnotDirection::Both,
        false,
    );
}

#[test]
fn eligible_noop_keeps_geometry_history_selection_and_accepts_options() {
    let single = surface().try_bezier_patches().unwrap().remove(0);
    let (mut d, id) = selected(Geometry::NurbsSurface(single));
    let r = CommandRegistry::with_builtins();
    let before = d.object(id).unwrap().clone();
    let history = d.undo_label().map(str::to_owned);
    r.execute(&mut d, "ConvertToSingleSpans Direction=V DeleteInput=Yes")
        .unwrap();
    assert_eq!(d.object(id), Some(&before));
    assert!(d.is_selected(id));
    assert_eq!(d.undo_label(), history.as_deref());
    check_preference(&r, SurfaceKnotDirection::V, true);
}

#[test]
fn invalid_options_and_ineligible_selection_do_not_change_preferences_or_history() {
    let r = CommandRegistry::with_builtins();
    let (mut d, _) = selected(Geometry::NurbsSurface(surface()));
    r.execute(&mut d, "ConvertToSingleSpans Direction=U DeleteInput=Yes")
        .unwrap();
    let (mut point, id) = selected(Geometry::Point(Point3::try_new(0., 0., 0.).unwrap()));
    let history = point.undo_label().map(str::to_owned);
    for cmd in [
        "ConvertToSingleSpans Direction=V DeleteInput=No",
        "ConvertToSingleSpans Direction=V Other=Both",
        "ConvertToSingleSpans Direction=U Direction=V",
        "ConvertToSingleSpans DeleteInput=No DeleteInput=Yes",
        "ConvertToSingleSpans DeleteInput=Maybe",
    ] {
        assert!(r.execute(&mut point, cmd).is_err());
        assert_eq!(point.undo_label(), history.as_deref());
        assert!(point.is_selected(id));
    }
    check_preference(&r, SurfaceKnotDirection::U, true);
}

#[test]
fn trimmed_surfaces_drop_trims_but_mixed_points_remain_selected() {
    let b = Brep::try_rectangular_surface_face_with_orientation(
        surface(),
        -1.0..=3.0,
        11.0..=17.0,
        true,
        Tolerance::DEFAULT,
    )
    .unwrap();
    let (mut d, id) = selected(Geometry::Brep(b));
    let point = d
        .add_geometry(Geometry::Point(Point3::try_new(20., 0., 0.).unwrap()))
        .unwrap();
    d.select_objects_direct([point], SelectionMode::Add)
        .unwrap();
    CommandRegistry::with_builtins()
        .execute(
            &mut d,
            "ConvertToSingleSpans Direction=Both DeleteInput=Yes",
        )
        .unwrap();
    assert!(d.object(id).is_none());
    assert_eq!(d.selected_object_ids().collect::<Vec<_>>(), [point]);
    assert_eq!(
        d.objects()
            .filter(|o| matches!(o.geometry(), Geometry::NurbsSurface(_)))
            .count(),
        4
    );
}
