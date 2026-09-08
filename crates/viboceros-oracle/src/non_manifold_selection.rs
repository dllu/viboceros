//! Actual selection command over open, closed, and non-manifold topology.

use super::ProbeError;
use serde_json::{Value, json};
use viboceros_command::CommandRegistry;
use viboceros_document::{Document, Geometry, SelectionMode};
use viboceros_geometry::{Brep, Point3, Tolerance, TriangleMesh};

pub(super) fn run(
    as_brep: bool,
    preselect: bool,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    let vertices = [
        [0., 0., 0.],
        [1., 0., 0.],
        [0., 1., 0.],
        [0., 0., 1.],
        [0., -1., 1.],
    ]
    .map(Point3::try_from)
    .into_iter()
    .collect::<Result<Vec<_>, _>>()?;
    let tetra = vec![[0, 2, 1], [0, 1, 3], [0, 3, 2], [1, 2, 3]];
    let mut hanging = tetra.clone();
    hanging.push([0, 1, 4]);
    let mut document = Document::new(tolerance);
    let mut ids = Vec::new();
    for faces in [vec![[0, 2, 1]], tetra, hanging] {
        let mesh = TriangleMesh::try_new(vertices.clone(), faces, tolerance)?;
        let geometry = if as_brep {
            Geometry::Brep(Brep::try_from_mesh(&mesh, true, tolerance)?)
        } else {
            Geometry::Mesh(mesh)
        };
        ids.push(document.add_geometry(geometry)?);
    }
    if preselect {
        document.select_object(ids[0], SelectionMode::Replace)?;
    }
    let original = document.objects().cloned().collect::<Vec<_>>();
    let undo = document.undo_label().map(str::to_owned);
    let redo = document.redo_label().map(str::to_owned);
    CommandRegistry::with_builtins().execute(&mut document, "SelNonManifold")?;
    if document.objects().cloned().collect::<Vec<_>>() != original
        || document.undo_label() != undo.as_deref()
        || document.redo_label() != redo.as_deref()
    {
        return Err(ProbeError::FixtureInvariant(
            "selection changed source objects or history",
        ));
    }
    let selected = ids
        .iter()
        .enumerate()
        .filter_map(|(i, id)| document.is_selected(*id).then_some(i))
        .collect::<Vec<_>>();
    Ok((json!({"selected": selected}), 0))
}

#[cfg(test)]
mod tests {
    #[test]
    fn fixture_checks_additive_mesh_and_brep_selection() {
        let request = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/non_manifold_selection.json"
        ))
        .unwrap();
        let response = crate::run_request(&request).unwrap();
        assert_eq!(response.results.len(), 4);
        for result in response.results {
            let expected = if result.id.ends_with("additive") {
                serde_json::json!([0, 2])
            } else {
                serde_json::json!([2])
            };
            assert_eq!(result.value["selected"], expected);
        }
    }
}
