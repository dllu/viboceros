//! Independent sources and real command/session history for SubD direction.
use super::*;
use viboceros_command::direction::{DirectionEdit, DirectionSession};
use viboceros_document::ObjectAttributes;

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Fixture {
    pub scene: String,
    pub command: String,
    pub flat: bool,
    pub preselect: bool,
    pub mixed: bool,
    pub steps: Vec<String>,
    pub cancel: bool,
    #[serde(default)]
    pub bare: bool,
}

pub fn source(scene: &str, tolerance: Tolerance) -> Result<Geometry, ProbeError> {
    let geometry = if matches!(scene, "bent_grid" | "pentagon" | "fan7") {
        subd_area::source(scene, tolerance)?
    } else {
        subd_volume::source(scene, tolerance)?
    };
    let Geometry::SubD(net) = geometry else {
        unreachable!()
    };
    // Match the public mesh-conversion input route, independently constructing
    // the mesh from our recipes. Component IDs then have the same meaning in
    // both implementations; raw AddVertex/AddFace assigns different edge IDs.
    let mut faces = Vec::new();
    let mut ngons = Vec::new();
    for f in net.faces() {
        match *f.as_slice() {
            [a, b, c] => faces.push(viboceros_geometry::MeshFace::Triangle([a, b, c])),
            [a, b, c, d] => faces.push(viboceros_geometry::MeshFace::Quad([a, b, c, d])),
            _ => {
                let start = faces.len() as u32;
                for i in 1..f.len() - 1 {
                    faces.push(viboceros_geometry::MeshFace::Triangle([
                        f[0],
                        f[i],
                        f[i + 1],
                    ]));
                }
                ngons.push(viboceros_geometry::MeshNgon::from_parts(
                    f.clone(),
                    (start..faces.len() as u32).collect(),
                ));
            }
        }
    }
    let mesh = TriangleMesh::try_from_face_records(net.control_points().to_vec(), faces)?
        .try_with_ngons(ngons)?;
    let mut subd = viboceros_geometry::SubD::try_from_mesh(&mesh, Default::default(), tolerance)?;
    let corners = net
        .vertex_tags()
        .iter()
        .enumerate()
        .filter(|(_, t)| **t == 3)
        .map(|(i, _)| (i, 3))
        .collect::<Vec<_>>();
    subd = subd.try_with_vertex_tags(&corners)?;
    let creases = net
        .edges()
        .iter()
        .zip(net.edge_tags())
        .filter(|(_, tag)| **tag == 2)
        .map(|(pair, _)| {
            let mut pair = *pair;
            pair.sort_unstable();
            pair
        })
        .collect::<BTreeSet<_>>();
    let tags = subd
        .edges()
        .iter()
        .enumerate()
        .filter_map(|(i, pair)| {
            let mut pair = *pair;
            pair.sort_unstable();
            creases.contains(&pair).then_some((i, 2))
        })
        .collect::<Vec<_>>();
    subd = subd.try_with_edge_tags(&tags)?;
    Ok(Geometry::SubD(subd))
}

pub fn definition(geometry: &Geometry) -> Result<Value, ProbeError> {
    Ok(match geometry {
        Geometry::SubD(s) => {
            let ids = s.component_ids()?;
            json!({"kind":"subd","points":s.control_points().iter().map(|p|p.to_array()).collect::<Vec<_>>(),
                "vertex_tags":s.vertex_tags(),"vertex_ids":ids.vertices,"edges":s.edges(),
                "edge_tags":s.edge_tags(),"edge_ids":ids.edges,"faces":s.faces(),"face_ids":ids.faces})
        }
        Geometry::Line(l) => {
            json!({"kind":"line","points":[l.start().to_array(),l.end().to_array()]})
        }
        _ => {
            return Err(ProbeError::FixtureInvariant(
                "owned direction source required",
            ));
        }
    })
}

pub fn snapshot(doc: &Document, ids: &[ObjectId]) -> Result<Value, ProbeError> {
    let sources = ids.iter().map(|&id| {
        let object = doc.object(id).ok_or(ProbeError::FixtureInvariant("direction source identity changed"))?;
        Ok(json!({"geometry":definition(object.geometry())?,"selected":doc.is_selected(id),
            "name":object.attributes().name(),"groups":doc.groups().filter(|g|g.members().any(|m|m==id)).count()}))
    }).collect::<Result<Vec<Value>, ProbeError>>()?;
    Ok(
        json!({"sources":sources,"objects":doc.objects().len(),"points":doc.objects().filter_map(|o|match o.geometry(){Geometry::Point(p)=>Some(p.to_array()),_=>None}).collect::<Vec<_>>()}),
    )
}

pub(super) fn run(
    f: &Fixture,
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    if iterations != 1
        || !matches!(f.command.as_str(), "Dir" | "Flip")
        || f.steps
            .iter()
            .any(|s| !matches!(s.as_str(), "Flip" | "FlipAll"))
        || f.steps.len() > 2
        || (f.command == "Flip" && (!f.steps.is_empty() || f.cancel))
    {
        return Err(ProbeError::FixtureInvariant(
            "owned direction workflow required",
        ));
    }
    measure_probe(iterations, || -> Result<Value, ProbeError> {
        let mut doc = Document::new(tolerance);
        let registry = CommandRegistry::with_builtins();
        let id = doc.add_geometry_with_attributes(
            source(&f.scene, tolerance)?,
            ObjectAttributes::on_layer(doc.current_layer_id()).with_name("Direction source"),
        )?;
        let mut ids = vec![id];
        if f.mixed {
            ids.push(doc.add_geometry_with_attributes(
                Geometry::Line(LineSegment::try_new(
                    Point3::try_new(20., 0., 0.)?,
                    Point3::try_new(23., 1., 0.)?,
                    tolerance,
                )?),
                ObjectAttributes::on_layer(doc.current_layer_id()).with_name("Direction curve"),
            )?);
        }
        doc.select_objects_direct(ids.iter().copied(), SelectionMode::Replace)?;
        registry.execute(&mut doc, "Group OwnedDirectionGroup")?;
        if f.flat {
            registry.execute(&mut doc, "SubDDisplayToggle")?;
        }
        doc.clear_selection();
        doc.clear_history()?;
        registry.execute(&mut doc, "Point 40,50,60")?;
        registry.execute(&mut doc, "Undo")?;
        if f.preselect {
            doc.select_objects_direct(ids.iter().copied(), SelectionMode::Add)?;
        }
        let before = snapshot(&doc, &ids)?;
        if !f.preselect {
            doc.set_transient_selection(ids.iter().copied())?;
        }
        if f.command == "Dir" {
            let mut session = DirectionSession::new(&doc, !f.preselect)?;
            for _ in &f.steps {
                session.apply(&doc, DirectionEdit::Flip, None)?;
            }
            if f.cancel {
                session.cancel(&mut doc)?;
            } else {
                session.accept(&mut doc)?;
            }
        } else if f.preselect {
            registry.execute(&mut doc, "Flip")?;
        } else {
            registry.execute_postselected(&mut doc, "Flip", Default::default())?;
        }
        let after = snapshot(&doc, &ids)?;
        let changed = before["sources"]
            .as_array()
            .unwrap()
            .iter()
            .zip(after["sources"].as_array().unwrap())
            .any(|(a, b)| a["geometry"] != b["geometry"]);
        let (undo, redo) = if changed {
            registry.execute(&mut doc, "Undo")?;
            let undo = snapshot(&doc, &ids)?;
            registry.execute(&mut doc, "Redo")?;
            (undo, snapshot(&doc, &ids)?)
        } else {
            registry.execute(&mut doc, "Redo")?;
            let redo = snapshot(&doc, &ids)?;
            registry.execute(&mut doc, "Undo")?;
            (snapshot(&doc, &ids)?, redo)
        };
        // Rhino reports Nothing for accepted SubD-only Dir, including changed
        // geometry. Keep semantic completion separate from that raw diagnostic.
        Ok(
            json!({"completed":!f.cancel,"changed":changed,"before":before,"after":after,"undo":undo,"redo":redo}),
        )
    })
}
