mod open;
use super::*;
use crate::boolean_union::tests::{Boundary, compare, regions, setup, snapshot, witnesses};
use serde_json::Value;
#[test]
fn boolean_two_replays_native_cycle_geometry_identity_metadata_and_history() {
    let q: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/observations/boolean_two_command.json"
    ))
    .unwrap();
    for r in q["results"].as_array().unwrap() {
        let v = &r["value"];
        if v["cancel"] == true {
            continue;
        }
        let (mut doc, ids, layers, groups) = setup(&v["before"]);
        doc.clear_history().unwrap();
        let mode = Mode::ALL[v["cycles"].as_u64().unwrap() as usize % 5];
        let registry = CommandRegistry::with_builtins();
        let result = registry.execute(
            &mut doc,
            &format!(
                "Boolean2Objects Sources={},{} Mode={} DeleteInput={}",
                ids[0],
                ids[1],
                mode.name(),
                if v["delete"] == true { "Yes" } else { "No" }
            ),
        );
        assert_eq!(result.is_ok(), v["command"]["success"] == true);
        let actual = snapshot(&doc, &ids, &layers, &groups);
        compare(
            &actual,
            &v["command"]["after_script"],
            v["case"].as_str().unwrap(),
        );
        for (o, n) in doc
            .objects()
            .zip(v["command"]["after_script"].as_array().unwrap())
        {
            let Geometry::Brep(b) = o.geometry() else {
                panic!()
            };
            let a = regions(b);
            let expected = serde_json::from_value::<Boundary>(n["face_regions"].clone()).unwrap();
            witnesses(&a, &expected, "Boolean2Objects");
            witnesses(&expected, &a, "Boolean2Objects");
        }
        if result.is_err() {
            assert!(!doc.can_undo());
            continue;
        }
        registry.execute(&mut doc, "Undo").unwrap();
        compare(
            &snapshot(&doc, &ids, &layers, &groups),
            &v["undo"]["after_script"],
            "undo",
        );
        registry.execute(&mut doc, "Redo").unwrap();
        compare(
            &snapshot(&doc, &ids, &layers, &groups),
            &v["redo"]["after_script"],
            "redo",
        );
    }
}

#[test]
fn prepared_choices_reuse_geometry_and_reject_invalid_acceptance_atomically() {
    use crate::boolean_union::tests::box_brep;
    let a = box_brep([[0., 2.]; 3]);
    let b = box_brep([[1., 3.]; 3]);
    let prepared = prepare(&a, &b, Tolerance::DEFAULT).unwrap();
    assert!(Arc::ptr_eq(
        &prepared.get(Mode::Union),
        &prepared.get(Mode::Union)
    ));
    assert_eq!(
        Mode::ALL
            .into_iter()
            .map(|m| prepared
                .get(m)
                .iter()
                .map(|p| p.brep.signed_volume(Tolerance::DEFAULT).unwrap())
                .sum::<f64>())
            .collect::<Vec<_>>(),
        [15., 1., 7., 7., 14.]
    );
    let mut doc = Document::default();
    let ids = [
        doc.add_geometry(Geometry::Brep(a)).unwrap(),
        doc.add_geometry(Geometry::Brep(b)).unwrap(),
    ];
    doc.clear_history().unwrap();
    let before = doc.objects().cloned().collect::<Vec<_>>();
    assert!(accept(&mut doc, ids, &[], true).is_err());
    let bad = Piece {
        owner: 2,
        retain_geometry_user_text: true,
        brep: prepared.get(Mode::Union)[0].brep.clone(),
    };
    assert!(accept(&mut doc, ids, &[bad], true).is_err());
    assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
    assert!(!doc.can_undo());
}
#[test]
fn boolean_two_script_syntax_and_unsupported_geometry_fail_without_edits() {
    let registry = CommandRegistry::with_builtins();
    let mut doc = Document::default();
    registry.execute(&mut doc, "Box 0,0 2,2 2").unwrap();
    registry.execute(&mut doc, "Sphere 1,1 2").unwrap();
    let ids = doc.objects().map(|o| o.id()).collect::<Vec<_>>();
    doc.clear_history().unwrap();
    let before = doc.objects().cloned().collect::<Vec<_>>();
    for input in [
        format!(
            "Boolean2Objects Sources={},{} Mode=NoSuchMode",
            ids[0], ids[1]
        ),
        format!(
            "Boolean2Objects Sources={},{} DeleteInput=Maybe",
            ids[0], ids[1]
        ),
        format!("Boolean2Objects Sources={},{}", ids[0], ids[1]),
    ] {
        assert!(registry.execute(&mut doc, &input).is_err());
        assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
        assert!(!doc.can_undo());
    }
}

#[test]
fn boolean_two_coplanar_replays_overlap_boundary_categories_metadata_and_history() {
    let q: Value = serde_json::from_str(include_str!(
        "../../../../tools/rhino_oracle/observations/boolean_two_coplanar.json"
    ))
    .unwrap();
    open::replay(&q);
}
