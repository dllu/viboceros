//! Native custom sphere packing, transient toggles and complete command outputs.
use serde_json::Value;
use std::io::Read;
use viboceros_command::{CommandContext, CommandRegistry};
use viboceros_document::{Document, Geometry, ObjectAttributes, ObjectId};
use viboceros_geometry::{LineSegment, Point3, SubD, SubDNurbsContinuity, Tolerance};
use viboceros_io::{ThreeDmGeometry, read_3dm_file};
#[path = "../../viboceros-geometry/tests/support/nurbs_common_basis.rs"]
mod basis;
#[path = "../../viboceros-geometry/tests/support/positive_tensor.rs"]
mod bound;
#[path = "support/subd_nurbs_images.rs"]
mod complete;
#[path = "../../viboceros-geometry/tests/support/subd_packed_images.rs"]
mod images;
#[path = "../../viboceros-geometry/tests/support/subd_to_nurbs.rs"]
#[allow(dead_code)]
mod native;
#[path = "../../viboceros-geometry/tests/support/nurbs_surface.rs"]
mod surface;
#[path = "../../viboceros-geometry/tests/support/subd_packed_topology.rs"]
mod topology;
fn source(row: &Value) -> SubD {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("source.3dm");
    let bytes = row["archive"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_u64().unwrap() as u8)
        .collect::<Vec<_>>();
    std::fs::write(&path, bytes).unwrap();
    let model = read_3dm_file(path, Tolerance::DEFAULT).unwrap();
    let ThreeDmGeometry::SubD(s) = &model.objects[0].geometry else {
        panic!()
    };
    s.clone()
}
fn check(doc: &Document, ids: &[ObjectId], expected: &Value) {
    assert_eq!(doc.objects().len(), expected.as_array().unwrap().len());
    for (object, row) in doc.objects().zip(expected.as_array().unwrap()) {
        assert_eq!(
            ids.iter().position(|&id| id == object.id()),
            row["source"].as_u64().map(|v| v as usize)
        );
        assert_eq!(
            doc.is_selected(object.id()),
            row["selected"].as_bool().unwrap()
        );
        assert_eq!(
            object.attributes().name().unwrap_or(""),
            row["name"].as_str().unwrap_or("")
        );
        if let Geometry::SubD(s) = object.geometry() {
            let native = source(row);
            assert_eq!(
                s.face_packs()
                    .unwrap()
                    .iter()
                    .map(|p| p.unwrap().id)
                    .collect::<Vec<_>>(),
                native
                    .face_packs()
                    .unwrap()
                    .iter()
                    .map(|p| p.unwrap().id)
                    .collect::<Vec<_>>()
            );
            assert_eq!(s.control_points(), native.control_points());
            assert_eq!(s.faces(), native.faces());
            assert_eq!(s.vertex_tags(), native.vertex_tags());
            assert_eq!(s.edge_tags(), native.edge_tags());
        }
    }
}
#[test]
fn native_face_pack_actions_preserve_sources_and_undo_complete_conversion() {
    let commands = CommandRegistry::with_builtins();
    let mut count = 0;
    for bytes in [
        include_bytes!(
            "../../../tools/rhino_oracle/observations/subd_nurbs_packs_discovery.json.gz"
        )
        .as_slice(),
        include_bytes!("../../../tools/rhino_oracle/observations/subd_nurbs_packs.json.gz")
            .as_slice(),
    ] {
        let mut raw = Vec::new();
        flate2::read::GzDecoder::new(bytes)
            .read_to_end(&mut raw)
            .unwrap();
        let data: Value = serde_json::from_slice(&raw).unwrap();
        for row in data["results"].as_array().unwrap() {
            count += 1;
            let id = row["id"].as_str().unwrap();
            let v = &row["value"];
            println!("{id}");
            let mut doc = Document::default();
            let mut ids = Vec::new();
            let original = source(&v["before"][0]);
            for r in v["before"].as_array().unwrap() {
                let geometry = match r["kind"].as_str().unwrap() {
                    "SubD" => Geometry::SubD(original.clone()),
                    "Point" => Geometry::Point(Point3::try_new(20., 0., 0.).unwrap()),
                    "LineCurve" => Geometry::Line(
                        LineSegment::try_new(
                            Point3::try_new(30., 0., 0.).unwrap(),
                            Point3::try_new(34., 1., 0.).unwrap(),
                            Tolerance::DEFAULT,
                        )
                        .unwrap(),
                    ),
                    _ => panic!(),
                };
                ids.push(
                    doc.add_geometry_with_attributes(
                        geometry,
                        ObjectAttributes::on_layer(doc.current_layer_id())
                            .with_name(r["name"].as_str().unwrap_or("")),
                    )
                    .unwrap(),
                );
            }
            doc.clear_history().unwrap();
            let post =
                id.ends_with("_True") && id.starts_with("mixed_") || id == "post_force_cancel";
            let cancel = v["success"] == false;
            let force = id == "force_accept"
                || id.starts_with("mixed_")
                || id == "force_unpacked"
                || id == "force_options_cancel"
                || id == "post_force_cancel"
                || id == "force_cancel";
            let packed = id != "force_unpacked";
            let show = id == "show_cancel" || id == "show_accept" || id.starts_with("mixed_");
            let delete = id.starts_with("mixed_True_");
            let selected = if post {
                ids.iter()
                    .enumerate()
                    .rev()
                    .filter(|(i, _)| *i != 1)
                    .map(|(_, id)| *id)
                    .collect()
            } else {
                ids.clone()
            };
            doc.set_transient_selection(selected).unwrap();
            let before = doc.objects().cloned().collect::<Vec<_>>();
            let command = format!(
                "ToNURBS DeleteInputObjects={} Faces={} ShowFacePacks={} ForceUpdateFacePacks={}",
                if delete { "Yes" } else { "No" },
                if packed { "Packed" } else { "Unpacked" },
                if show { "Yes" } else { "No" },
                if force { "Yes" } else { "No" }
            );
            let mut prompt = commands.object_selection_prompt(&command).unwrap().unwrap();
            commands.accept_object_selection_options(&prompt).unwrap();
            let mut preview = viboceros_command::to_nurbs::FacePackPreview::default();
            let view = preview.view(&doc, Some(&prompt)).unwrap();
            assert_eq!(view.is_some(), show && packed);
            if let Some(view) = view {
                let colors = view.colors(doc.object(ids[0]).unwrap()).unwrap();
                assert_eq!(colors.len(), 24);
                assert_eq!(
                    colors
                        .iter()
                        .collect::<std::collections::BTreeSet<_>>()
                        .len(),
                    if force { 6 } else { 2 }
                );
                assert!(std::sync::Arc::ptr_eq(
                    &colors,
                    &preview
                        .view(&doc, Some(&prompt))
                        .unwrap()
                        .unwrap()
                        .colors(doc.object(ids[0]).unwrap())
                        .unwrap()
                ));
            }
            assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
            if cancel {
                if post {
                    doc.clear_selection();
                }
                check(&doc, &ids, &v["settled"]);
                assert!(!doc.can_undo());
            } else {
                if post {
                    commands
                        .execute_postselected(&mut doc, &command, CommandContext::default())
                        .unwrap();
                } else {
                    commands.execute(&mut doc, &command).unwrap();
                }
                check(&doc, &ids, &v["settled"]);
                let conversion = if force {
                    original.try_repacked_faces().unwrap()
                } else {
                    original.clone()
                };
                let brep = doc
                    .objects()
                    .find_map(|o| {
                        if let Geometry::Brep(b) = o.geometry() {
                            Some(b)
                        } else {
                            None
                        }
                    })
                    .unwrap();
                let target = v["settled"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|r| r["kind"] == "Brep")
                    .unwrap();
                complete::check(
                    brep,
                    &conversion,
                    packed,
                    SubDNurbsContinuity::G1x,
                    &target["brep"],
                );
                let after = doc.objects().cloned().collect::<Vec<_>>();
                doc.undo().unwrap();
                assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
                check(&doc, &ids, &v["undo"]);
                doc.redo().unwrap();
                assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), after);
                check(&doc, &ids, &v["redo"]);
            }
            let fresh = commands
                .object_selection_prompt("ToNURBS")
                .unwrap()
                .unwrap();
            assert!(fresh.menus.is_empty());
            prompt = commands
                .object_selection_confirmation(&doc, &fresh)
                .unwrap()
                .unwrap_or(fresh);
            assert!(
                prompt
                    .menus
                    .iter()
                    .flat_map(|m| &m.options)
                    .filter(|o| o.name == "ShowFacePacks" || o.name == "ForceUpdateFacePacks")
                    .all(|o| !o.value)
            );
        }
    }
    assert_eq!(count, 14);
}
