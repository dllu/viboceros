use viboceros_command::{CommandRegistry, ExtractSurfaceSelection};
use viboceros_document::Geometry;
use viboceros_oracle::subd_join::snapshot;
#[path = "support/subd_join.rs"]
mod common;
#[path = "support/subd_extract.rs"]
mod support;
use support::{expected as without_components, fixture};
#[test]
fn subd_extract_matches_native_geometry_groups_selection_order_and_history() {
    let sources = support::sources();
    for (op, native) in ["subd_extract_command", "subd_extract_details"]
        .into_iter()
        .flat_map(support::cases_for)
    {
        let id = op["id"].as_str().unwrap();
        assert_eq!(native["success"], true, "{id}");
        let (mut doc, ids) = fixture(&native, &sources[op["scene"].as_str().unwrap()]);
        support::compare(
            &snapshot(&doc, &ids),
            &without_components(native["before"].clone()),
            &format!("{id}:before"),
        );
        let picks = native["input"]["faces"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| {
                (
                    ids[p[0].as_u64().unwrap() as usize],
                    p[1].as_u64().unwrap() as usize,
                )
            });
        ExtractSurfaceSelection::prepare_with_subd_corners(
            &doc,
            picks,
            op["copy"].as_bool().unwrap(),
            native["input"]["current"].as_bool().unwrap_or(false),
            op["corners"].as_bool().unwrap(),
        )
        .unwrap()
        .commit(&mut doc)
        .unwrap();
        support::compare(
            &snapshot(&doc, &ids),
            &without_components(native["after"].clone()),
            &format!("{id}:after"),
        );
        if op["scene"] == "all" {
            assert!(!doc.can_undo());
            continue;
        }
        doc.undo().unwrap();
        support::compare(
            &snapshot(&doc, &ids),
            &without_components(native["undo"].clone()),
            &format!("{id}:undo"),
        );
        doc.redo().unwrap();
        support::compare(
            &snapshot(&doc, &ids),
            &without_components(native["redo"].clone()),
            &format!("{id}:redo"),
        );
    }
}
#[test]
fn subd_extract_typed_indices_options_and_native_3dm_roundtrip() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("extracted.3dm");
    let sources = support::sources();
    for (op, native) in ["subd_extract_command", "subd_extract_details"]
        .into_iter()
        .flat_map(support::cases_for)
    {
        if op["scene"] != "single" {
            continue;
        }
        let (mut doc, ids) = fixture(&native, &sources[op["scene"].as_str().unwrap()]);
        doc.select_objects_direct(
            ids.iter().copied(),
            viboceros_document::SelectionMode::Replace,
        )
        .unwrap();
        CommandRegistry::with_builtins()
            .execute(
                &mut doc,
                &format!(
                    "ExtractSrf Face=0 Copy={} SubDAddCorners={}",
                    if op["copy"] == true { "Yes" } else { "No" },
                    if op["corners"] == true { "Yes" } else { "No" }
                ),
            )
            .unwrap();
        let soft = doc.objects().find_map(|o| {
            let Geometry::SubD(s) = o.geometry() else {
                panic!()
            };
            s.edge_face_counts()
                .iter()
                .enumerate()
                .find(|&(i, &n)| n == 2 && s.edge_tags()[i] != 2)
                .map(|(edge, _)| {
                    (
                        o.id(),
                        s.try_with_stored_edge_sharpness(&[(edge, [1.25, 2.5])])
                            .unwrap()
                            .0,
                    )
                })
        });
        if let Some((id, s)) = soft {
            doc.replace_object_geometries([(id, Geometry::SubD(s))])
                .unwrap();
        }
        doc.clear_selection();
        viboceros_command::save_3dm_with_named_views(&doc, path.to_str().unwrap(), &[]).unwrap();
        let (restored, _, _, _, _) =
            viboceros_command::open_3dm_with_views_and_cplanes(path.to_str().unwrap()).unwrap();
        support::compare(
            &snapshot(&restored, &ids),
            &snapshot(&doc, &ids),
            op["id"].as_str().unwrap(),
        );
        for (before, after) in doc.objects().zip(restored.objects()) {
            let (Geometry::SubD(before), Geometry::SubD(after)) =
                (before.geometry(), after.geometry())
            else {
                panic!("editable SubD roundtrip")
            };
            assert_eq!(before.edge_sharpness(), after.edge_sharpness());
        }
    }
}

#[test]
fn subd_extract_corner_preferences_match_native_and_survive_history() {
    let sources = support::sources();
    let cases = support::cases_for("subd_extract_preferences");
    let registry = CommandRegistry::with_builtins();
    for (i, native) in cases[0].1.as_array().unwrap().iter().enumerate() {
        let (mut doc, ids) = fixture(native, &sources["single"]);
        doc.select_objects_direct(
            ids.iter().copied(),
            viboceros_document::SelectionMode::Replace,
        )
        .unwrap();
        registry
            .execute(
                &mut doc,
                match i {
                    0 => "ExtractSrf Face=0 SubDAddCorners=Yes",
                    2 => "ExtractSrf Face=0 SubDAddCorners=No",
                    _ => "ExtractSrf Face=0",
                },
            )
            .unwrap();
        support::compare(
            &snapshot(&doc, &ids),
            &without_components(native["after_script"].clone()),
            &format!("preferences {i}"),
        );
        doc.undo().unwrap();
        doc.redo().unwrap();
        assert_eq!(registry.extract_subd_corners(), i < 2);
        assert!(!CommandRegistry::with_builtins().extract_subd_corners());
        assert!(
            registry
                .execute(
                    &mut doc,
                    "ExtractSrf Face=0 SubDAddCorners=Yes SubDAddCorners=No"
                )
                .is_err()
        );
        assert_eq!(registry.extract_subd_corners(), i < 2);
    }
}
