use super::*;
use crate::boolean_union::tests::box_brep;
use serde_json::Value;

#[test]
fn curved_public_cycles_match_geometry_metadata_identity_and_history() {
    use crate::boolean_union::tests::{compare, metadata_snapshot};
    let capture: Value = serde_json::from_str(include_str!(
        "../../../../../tools/rhino_oracle/observations/boolean_two_curved.json"
    ))
    .unwrap();
    let tolerance = Tolerance::DEFAULT;
    let frame = CommandContext::default().construction_plane;
    let original = [
        Brep::try_cylinder(frame, 2., 0., 10., tolerance).unwrap(),
        box_brep([[-3., 3.], [-3., 3.], [4., 6.]]),
    ];
    let analytic_boundary = |p: Point3, mode: Mode| {
        let cylinder = (p.x().hypot(p.y()) - 2.).max(-p.z()).max(p.z() - 10.);
        let slab = (p.x().abs() - 3.)
            .max(p.y().abs() - 3.)
            .max(4. - p.z())
            .max(p.z() - 6.);
        let value = match mode {
            Mode::Union => cylinder.min(slab),
            Mode::Intersection => cylinder.max(slab),
            Mode::DifferenceAB => cylinder.max(-slab),
            Mode::DifferenceBA => slab.max(-cylinder),
            Mode::InverseIntersection => cylinder.max(-slab).min(slab.max(-cylinder)),
        };
        assert!(value.abs() < 1e-8, "{mode:?} boundary {p:?}: {value}");
    };
    let prepared = prepare(&original[0], &original[1], tolerance).unwrap();
    let pi = std::f64::consts::PI;
    let expected_volumes = [
        72. + 32. * pi,
        8. * pi,
        32. * pi,
        72. - 8. * pi,
        72. + 24. * pi,
    ];
    let expected_areas = [120. + 32. * pi, 16. * pi, 48. * pi, 120., 120. + 48. * pi];
    let mut physical: [Vec<(f64, f64, Point3)>; 5] = std::array::from_fn(|_| Vec::new());
    for mode in Mode::ALL {
        let pieces = prepared.get(mode);
        for piece in pieces.iter() {
            let mass = piece.brep.volume_mass_properties(tolerance).unwrap();
            physical[mode.index()].push((
                mass.signed_volume().unwrap(),
                piece.brep.area(tolerance).unwrap(),
                mass.centroid().unwrap(),
            ));
        }
        let volume = physical[mode.index()].iter().map(|m| m.0).sum::<f64>();
        let area = physical[mode.index()].iter().map(|m| m.1).sum::<f64>();
        assert!(
            (volume - expected_volumes[mode.index()]).abs() < 1e-8,
            "{mode:?}: {volume}"
        );
        assert!(
            (area - expected_areas[mode.index()]).abs() < 1e-8,
            "{mode:?}: {area}"
        );
        for piece in pieces.iter() {
            for edge in piece.brep.edges() {
                let domain = edge.curve().domain();
                for i in 0..=32 {
                    analytic_boundary(
                        edge.curve()
                            .evaluate(
                                domain.start() + (domain.end() - domain.start()) * i as f64 / 32.,
                            )
                            .unwrap(),
                        mode,
                    );
                }
            }
            for face in piece.brep.faces() {
                for boundary in face.loops() {
                    for trim in boundary.trims() {
                        let domain = trim.curve().domain();
                        for i in 0..=32 {
                            let uv = trim
                                .curve()
                                .evaluate(
                                    domain.start()
                                        + (domain.end() - domain.start()) * i as f64 / 32.,
                                )
                                .unwrap();
                            analytic_boundary(
                                face.surface().evaluate(uv.x(), uv.y()).unwrap(),
                                mode,
                            );
                        }
                    }
                }
            }
        }
    }
    let mut partitions = Vec::new();
    for row in capture["results"].as_array().unwrap() {
        let value = &row["value"];
        let case = value["case"].as_str().unwrap();
        let mut doc = Document::default();
        let mut ids = Vec::new();
        let mut layers = Vec::new();
        for (index, geometry) in original.iter().enumerate() {
            let layer = doc
                .add_layer(format!("Source {index}"), ColorRgb::BLACK)
                .unwrap();
            let attributes = ObjectAttributes::on_layer(layer)
                .with_name(format!("source-{index}"))
                .with_object_color(ColorRgb::new(20 + index as u8, 40, 60))
                .try_with_user_text("Code", format!("attribute-{index}"))
                .unwrap();
            let id = doc
                .add_geometry_with_attributes(Geometry::Brep(geometry.clone()), attributes)
                .unwrap();
            doc.set_object_geometry_user_text([id], "Code", Some(&format!("geometry-{index}")))
                .unwrap();
            ids.push(id);
            layers.push(layer);
        }
        let mut groups = ids
            .iter()
            .map(|&id| doc.add_group(None, [id]).unwrap())
            .collect::<Vec<_>>();
        groups.push(doc.add_group(None, ids.iter().copied()).unwrap());
        doc.clear_history().unwrap();
        let before = doc.objects().cloned().collect::<Vec<_>>();
        if value["cancel"] == true {
            // Preparation and cycling do not write to the source document.
            for mode in Mode::ALL {
                assert!(!prepared.get(mode).is_empty());
            }
            assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
            continue;
        }
        let mode = Mode::ALL[value["cycles"].as_u64().unwrap() as usize % 5];
        let registry = CommandRegistry::with_builtins();
        registry
            .execute(
                &mut doc,
                &format!(
                    "Boolean2Objects Sources={},{} Mode={} DeleteInput={}",
                    ids[0],
                    ids[1],
                    mode.name(),
                    if value["delete"] == true { "Yes" } else { "No" }
                ),
            )
            .unwrap();
        let expected = &value["command"]["after_script"];
        let mut actual = metadata_snapshot(&doc, &ids, &layers, &groups);
        assert_eq!(
            actual.as_array().unwrap().len(),
            expected.as_array().unwrap().len(),
            "{case}: material body count"
        );
        let pieces = prepared.get(mode);
        let mut used = BTreeSet::new();
        let mut paired = Vec::new();
        for (index, own) in actual.as_array_mut().unwrap().iter_mut().enumerate() {
            let object = doc.objects().nth(index).unwrap();
            let Geometry::Brep(brep) = object.geometry() else {
                panic!("editable B-rep")
            };
            let (volume, area, centroid) = if brep == &original[0] {
                (40. * pi, 48. * pi, Point3::try_new(0., 0., 5.).unwrap())
            } else if brep == &original[1] {
                (72., 120., Point3::try_new(0., 0., 5.).unwrap())
            } else {
                // The public command must accept the same cached candidate
                // qualified above; avoid recomputing its scalar integrals.
                let piece = pieces
                    .iter()
                    .position(|p| &p.brep == brep)
                    .expect("command candidate geometry changed");
                physical[mode.index()][piece]
            };
            let (native_index, native) = expected
                .as_array()
                .unwrap()
                .iter()
                .enumerate()
                .find(|(i, n)| {
                    !used.contains(i)
                        && (volume - n["volume"].as_f64().unwrap()).abs() < 2e-6
                        && centroid
                            .to_array()
                            .into_iter()
                            .enumerate()
                            .all(|(i, c)| (c - n["centroid"][i].as_f64().unwrap()).abs() < 1e-8)
                })
                .expect("unmatched curved material body");
            used.insert(native_index);
            for (key, amount) in [("volume", volume), ("area", area)] {
                assert!(
                    (amount - native[key].as_f64().unwrap()).abs() < 2e-6,
                    "{case} {key}: {} != {}",
                    amount,
                    native[key]
                );
            }
            for (i, c) in centroid.to_array().into_iter().enumerate() {
                assert!(
                    (c - native["centroid"][i].as_f64().unwrap()).abs() < 1e-8,
                    "{case} centroid: {centroid:?} != {}",
                    native["centroid"]
                );
            }
            if index != native_index
                || own["source"] != native["source"]
                || own["faces"] != native["faces"]
                || own["edges"] != native["edges"]
            {
                partitions.push(serde_json::json!({"case":case,"actual_index":index,"native_index":native_index,"actual_source":own["source"],"native_source":native["source"],"actual":[own["faces"],own["edges"]],"native":[native["faces"],native["edges"]]}));
            }
            let mut native = native.clone();
            // Source-ID association follows runtime body order. Record its
            // difference explicitly; all other metadata remains mandatory.
            native["source"] = own["source"].clone();
            paired.push(native);
            own.as_object_mut().unwrap().remove("faces");
            own.as_object_mut().unwrap().remove("edges");
        }
        compare(&actual, &Value::Array(paired), case);
        let accepted = doc.objects().cloned().collect::<Vec<_>>();
        registry.execute(&mut doc, "Undo").unwrap();
        assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
        registry.execute(&mut doc, "Redo").unwrap();
        assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), accepted);
    }
    eprintln!(
        "Curved two-object representation: {}",
        serde_json::to_string_pretty(&partitions).unwrap()
    );
    let recorded: Value = serde_json::from_str(include_str!(
        "../../../../../docs/smlib-boolean-two-partitions.json"
    ))
    .unwrap();
    assert_eq!(serde_json::json!(partitions), recorded);
}

#[test]
fn tiny_shared_material_survives_and_all_choices_preserve_original_operands() {
    let first = box_brep([[0., 100_000.]; 3]);
    let second = box_brep([[99_999., 100_001.]; 3]);
    let before = (first.clone(), second.clone());
    let candidates = prepare(&first, &second, Tolerance::DEFAULT).unwrap();
    assert_eq!((first, second), before);
    let common = candidates.get(Mode::Intersection);
    assert_eq!(common.len(), 1);
    assert!((common[0].brep.signed_volume(Tolerance::DEFAULT).unwrap() - 1.).abs() < 1e-9);
    assert!(common[0].brep.vertices().iter().all(|v| {
        v.point()
            .to_array()
            .iter()
            .all(|c| (99_999. - 1e-8..=100_000. + 1e-8).contains(c))
    }));
    let remainder = candidates.get(Mode::DifferenceBA);
    assert_eq!(remainder.len(), 1);
    assert!((remainder[0].brep.signed_volume(Tolerance::DEFAULT).unwrap() - 7.).abs() < 1e-9);
    for mode in Mode::ALL {
        assert!(Arc::ptr_eq(&candidates.get(mode), &candidates.get(mode)));
        assert!(candidates.get(mode).iter().all(|p| p.brep.is_solid()));
    }
}

#[test]
fn invalid_or_noninteracting_operands_fail_without_candidates_or_source_changes() {
    let first = box_brep([[0., 2.]; 3]);
    for second in [
        first.clone(),
        box_brep([[3., 4.]; 3]),
        box_brep([[0.5, 1.5]; 3]),
        box_brep([[2., 3.]; 3]),
    ] {
        let before = (first.clone(), second.clone());
        assert!(prepare(&first, &second, Tolerance::DEFAULT).is_err());
        assert_eq!((&first, &second), (&before.0, &before.1));
    }
    let invalid = Brep::try_disjoint_union(
        vec![first.clone(), box_brep([[1., 3.]; 3])],
        Tolerance::DEFAULT,
    )
    .unwrap();
    let before = invalid.clone();
    assert!(prepare(&invalid, &first, Tolerance::DEFAULT).is_err());
    assert_eq!(invalid, before);
}
