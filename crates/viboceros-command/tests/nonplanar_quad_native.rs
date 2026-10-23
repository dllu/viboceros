//! Independent public SDK references for all six nonplanar quad split methods.
#[path = "support/mesh_conversion.rs"]
mod raw_mesh;
use serde_json::{Value, json};
use viboceros_command::CommandRegistry;
use viboceros_document::{Document, Geometry, SelectionMode};
use viboceros_geometry::{NonPlanarQuadCriterion, QuadSplitMethod, Tolerance};

#[test]
fn native_planarity_split_methods_channels_and_local_command_lifecycle_match() {
    let mut rows = Vec::new();
    for text in [
        include_str!("../../../tools/rhino_oracle/observations/nonplanar_quad_conversion_v2.json"),
        include_str!("../../../tools/rhino_oracle/observations/nonplanar_quad_ngons.json"),
        include_str!("../../../tools/rhino_oracle/observations/nonplanar_quad_angles.json"),
    ] {
        rows.extend(
            serde_json::from_str::<Value>(text).unwrap()["results"]
                .as_array()
                .unwrap()
                .iter()
                .cloned(),
        );
    }
    let methods = [
        (QuadSplitMethod::ShortestDiagonal, "ShortestDiagonal"),
        (QuadSplitMethod::LongestDiagonal, "LongestDiagonal"),
        (QuadSplitMethod::MinimizeArea, "MinimizeArea"),
        (QuadSplitMethod::MaximizeArea, "MaximizeArea"),
        (QuadSplitMethod::MinimumAngle, "MinimumAngle"),
        (QuadSplitMethod::MaximumAngle, "MaximumAngle"),
    ];
    for row in &rows {
        eprintln!("Nonplanar {}", row["id"]);
        let v = &row["value"];
        let input = &v["input"];
        assert_eq!(v["before"]["valid"], true);
        assert_eq!(v["after"]["valid"], true);
        let (source, before_invalid) = raw_mesh::mesh(&v["before"], false);
        assert!(before_invalid.is_empty());
        let (expected, invalid) = raw_mesh::mesh(&v["after"], true);
        let (method, name) = methods[input["method"].as_u64().unwrap() as usize - 1];
        let distance = input["distance"].as_f64();
        let angle = input["angle"].as_f64();
        let (criterion, options) = match (distance, angle) {
            (Some(d), Some(a)) => (
                NonPlanarQuadCriterion::Both {
                    distance: d,
                    angle: a,
                },
                format!("Mode=Both Distance={d:.17e} Angle={:.17e}", a.to_degrees()),
            ),
            (Some(d), None) => (
                NonPlanarQuadCriterion::Distance(d),
                format!("Mode=Distance Distance={d:.17e}"),
            ),
            (None, Some(a)) => (
                NonPlanarQuadCriterion::Angle(a),
                format!("Mode=Angle Angle={:.17e}", a.to_degrees()),
            ),
            _ => panic!(),
        };
        let (actual, converted) = source
            .triangulate_nonplanar_quads(criterion, method, Tolerance::DEFAULT)
            .unwrap();
        assert_eq!(actual, expected, "kernel records/channels");
        assert_eq!(converted, v["converted"].as_u64().unwrap() as usize);
        let registry = CommandRegistry::with_builtins();
        let mut doc = Document::default();
        let id = doc.add_geometry(Geometry::Mesh(source.clone())).unwrap();
        doc.set_object_names([(id, Some("Nonplanar reference".to_owned()))])
            .unwrap();
        doc.set_object_user_text([id], "oracle", row["id"].as_str())
            .unwrap();
        doc.add_group(Some("Quad group".to_owned()), [id]).unwrap();
        registry.execute(&mut doc, "Point 50,50,50").unwrap();
        doc.undo().unwrap();
        doc.select_objects_direct([id], SelectionMode::Replace)
            .unwrap();
        let attributes = doc.object(id).unwrap().attributes().clone();
        let groups = doc.object(id).unwrap().group_ids().to_vec();
        registry
            .execute(
                &mut doc,
                &format!("TriangulateNonPlanarQuads {options} SplitMethod={name}"),
            )
            .unwrap();
        assert_eq!(
            doc.object(id).unwrap().geometry(),
            &Geometry::Mesh(expected.clone()),
            "scripted command"
        );
        assert_eq!(doc.object(id).unwrap().attributes(), &attributes);
        assert_eq!(doc.object(id).unwrap().group_ids(), groups);
        assert!(doc.selected_object_ids().next().is_none());
        assert_eq!(doc.undo_label(), Some("TriangulateNonPlanarQuads"));
        assert!(doc.redo_label().is_none());
        doc.undo().unwrap();
        assert_eq!(doc.object(id).unwrap().geometry(), &Geometry::Mesh(source));
        doc.redo().unwrap();
        assert_eq!(
            doc.object(id).unwrap().geometry(),
            &Geometry::Mesh(expected.clone())
        );
        let empty_slots = v["after"]["ngons"]
            .as_array()
            .unwrap()
            .iter()
            .enumerate()
            .filter_map(|(i, g)| {
                (g["vertices"].as_array().unwrap().is_empty()
                    && g["faces"].as_array().unwrap().is_empty())
                .then_some(i)
            })
            .collect::<Vec<_>>();
        println!(
            "NONPLANAR_QUAD {}",
            json!({"id":row["id"],"qualified":true,"raw_faces":true,"channels":true,"vertices":expected.vertices().len(),"faces":expected.face_count(),"ngons":expected.ngons().len(),"converted":converted,"metadata":true,"selection":true,"history":true,"native_invalid_ngons":invalid,"native_empty_ngon_slots":empty_slots,"maximum_error":0.})
        );
    }
}
