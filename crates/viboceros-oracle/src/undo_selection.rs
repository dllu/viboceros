//! Whole-object selection replay across isolated command boundaries.
use super::*;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn isolated_command_history_matches_recorded_rhino() {
        let request: ProbeRequest = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/undo_selection.json"
        ))
        .unwrap();
        let observed: Value = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/observations/undo_selection.json"
        ))
        .unwrap();
        let response = run_request(&request).unwrap();
        let rows = observed["results"].as_array().unwrap();
        assert_eq!(rows.len(), 10);
        assert_eq!(response.results.len(), 10);
        for (actual, expected) in response.results.iter().zip(rows) {
            assert_eq!(actual.id, expected["id"].as_str().unwrap());
            assert_eq!(actual.value, expected["value"], "{}", actual.id);
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Fixture {
    kind: Kind,
    clear: bool,
}
#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
enum Kind {
    Move,
    Delete,
    Explode,
    DeleteFaces,
    ExtractMeshFaces,
}

pub(super) fn run(f: &Fixture, tolerance: Tolerance) -> Result<(Value, u64), ProbeError> {
    let mut d = Document::new(tolerance);
    let registry = CommandRegistry::with_builtins();
    let p = |x, y, z| Point3::try_new(x, y, z).unwrap();
    let geometry = match f.kind {
        Kind::DeleteFaces | Kind::ExtractMeshFaces => Geometry::Mesh(TriangleMesh::try_new(
            vec![
                p(0., 0., 0.),
                p(2., 0., 0.),
                p(0., 2., 0.),
                p(2., 2., 0.),
                p(1., 1., 1.),
            ],
            vec![[0, 1, 4], [1, 3, 4], [3, 2, 4], [2, 0, 4]],
            tolerance,
        )?),
        Kind::Explode => Geometry::Polyline(Polyline3::try_new(
            vec![
                p(0., 0., 0.),
                p(4., 0., 0.),
                p(4., 3., 0.),
                p(0., 3., 0.),
                p(0., 0., 0.),
            ],
            tolerance,
        )?),
        _ => Geometry::Line(LineSegment::try_new(
            p(0., 0., 0.),
            p(2., 0., 0.),
            tolerance,
        )?),
    };
    let source = d.add_geometry(geometry)?;
    let other = d.add_geometry(Geometry::Point(p(10., 10., 0.)))?;
    d.select_object(source, SelectionMode::Replace)?;
    registry.execute(
        &mut d,
        match f.kind {
            Kind::Move => "Move 0,0,0 0,1,0",
            Kind::Delete => "Delete",
            Kind::Explode => "Explode",
            Kind::DeleteFaces => "DeleteFaces Faces=0",
            Kind::ExtractMeshFaces => "ExtractMeshFaces Faces=0 MakeCopy=No",
        },
    )?;
    // Begin after the edit: typed native face arguments replace Rhino's actual
    // sub-object picker, whose pre-command selection has a different shape.
    let mut states = vec![snapshot(&d, source, other)];
    if f.clear {
        d.clear_selection();
    }
    d.select_object(other, SelectionMode::Add)?;
    states.push(snapshot(&d, source, other));
    for command in ["Undo", "Redo", "Undo"] {
        registry.execute(&mut d, command)?;
        states.push(snapshot(&d, source, other));
    }
    Ok((json!({"succeeded":true,"states":states}), 0))
}

fn snapshot(d: &Document, source: ObjectId, other: ObjectId) -> Value {
    let info = |id| {
        d.object(id).map(|o| json!({
        "selected":d.is_selected(id),
        "kind":match o.geometry() { Geometry::Mesh(_) => "Mesh", Geometry::Point(_) => "Point", _ => "Curve" },
        "faces":match o.geometry() { Geometry::Mesh(m) => Some(m.face_count()), _ => None },
    }))
    };
    json!({"source":info(source),"other":info(other),"count":d.objects().len(),
        "selected_outputs":d.selected_object_ids().filter(|id| *id != source && *id != other).count()})
}
