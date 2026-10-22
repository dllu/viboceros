//! Native seam geometry and one-edit document history checks.
use serde_json::Value;
use viboceros_command::CommandRegistry;
use viboceros_document::{ColorRgb, Document, Geometry, ObjectAttributes, SelectionMode};
#[path = "../../../viboceros-geometry/tests/support/mesh_channels.rs"]
mod channels;
#[path = "../../../viboceros-geometry/tests/support/mesh_equivalence.rs"]
mod equivalence;

pub fn check(name: &str, v: &Value, command: &str, history: &str) {
    check_with_geometry(name, v, command, history, true);
}

pub fn check_with_geometry(name: &str, v: &Value, command: &str, history: &str, strict: bool) {
    check_inner(name, v, command, history, strict, None);
}

// Reused by component qualifiers; other seam test crates use the strict entry.
#[allow(dead_code)]
pub fn check_with_differences(
    name: &str,
    v: &Value,
    command: &str,
    history: &str,
    expected: &[String],
) {
    check_inner(name, v, command, history, true, Some(expected));
}

fn check_inner(
    name: &str,
    v: &Value,
    command: &str,
    history: &str,
    strict: bool,
    expected: Option<&[String]>,
) {
    let source = channels::mesh(&v["before"]);
    let mut doc = Document::default();
    let registry = CommandRegistry::with_builtins();
    let attrs = ObjectAttributes::on_layer(doc.current_layer_id())
        .with_name("attributed source")
        .with_object_color(ColorRgb::new(31, 71, 119));
    let id = doc
        .add_geometry_with_attributes(Geometry::Mesh(source.clone()), attrs.clone())
        .unwrap();
    let group = doc.add_group(Some("assembly".into()), [id]).unwrap();
    doc.select_objects_direct([id], SelectionMode::Replace)
        .unwrap();
    doc.clear_history().unwrap();
    registry
        .execute(&mut doc, command)
        .unwrap_or_else(|error| panic!("{name} {command}: {error}"));
    assert_eq!(doc.objects().len(), 1);
    let object = doc.object(id).unwrap();
    assert_eq!(object.attributes(), &attrs);
    assert_eq!(object.group_ids(), [group]);
    assert!(doc.is_selected(id));
    let Geometry::Mesh(mesh) = object.geometry() else {
        panic!()
    };
    let differences = channels::differences(mesh, &v["after"]);
    let raw_order_qualified = differences.is_empty();
    if let Some(expected) = expected {
        assert_eq!(differences, expected, "{name}");
    } else {
        assert!(
            if strict {
                raw_order_qualified
            } else {
                equivalence::equivalent(mesh, &channels::mesh(&v["after"]))
            },
            "{name}"
        );
    }
    let result = object.geometry_snapshot().clone();
    assert_eq!(doc.undo_label(), Some(history));
    registry.execute(&mut doc, "Undo").unwrap();
    assert_eq!(doc.object(id).unwrap().geometry(), &Geometry::Mesh(source));
    registry.execute(&mut doc, "Redo").unwrap();
    assert_eq!(doc.object(id).unwrap().geometry_snapshot(), &result);
    assert_eq!(doc.object(id).unwrap().attributes(), &attrs);
    assert_eq!(doc.object(id).unwrap().group_ids(), [group]);
    assert!(doc.is_selected(id));
    if expected.is_some() {
        println!(
            "MESH_COMPONENT_WELD_COMMAND {}",
            serde_json::json!({"id":name,"command":command,"history":history,"qualified":raw_order_qualified,"differences":differences})
        );
    } else if strict {
        println!(
            "MESH_SEAM_COMMAND {}",
            serde_json::json!({"id":name,"command":command,"history":history,"qualified":true})
        );
    } else {
        println!(
            "MESH_SEAM_COMPARISON {}",
            serde_json::json!({"id":name,"command":command,"history":history,"qualified":true,"raw_order_qualified":raw_order_qualified})
        );
    }
}
