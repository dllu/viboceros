//! Model identities and selection, using the document and command component stores.
use super::*;
use std::collections::{HashMap, HashSet};
use viboceros_command::{ComponentSelectionKind, component_selection::ComponentPick};
use viboceros_document::{ControlPointId, Geometry, Object};

#[derive(Deserialize)]
pub(super) struct ComponentRef {
    object: String,
    kind: String,
    index: usize,
}

#[derive(Deserialize)]
pub(super) struct GripRef {
    object: String,
    index: usize,
}

impl ComponentRef {
    pub(super) fn pick(self) -> Result<ComponentPick> {
        Ok(ComponentPick {
            object: object_id(&self.object)?,
            kind: component_kind(&self.kind)?,
            index: self.index,
        })
    }
}

impl GripRef {
    pub(super) fn pick(self) -> Result<ControlPointId> {
        Ok(ControlPointId {
            object: object_id(&self.object)?,
            index: self.index,
        })
    }
}

pub(super) fn object_id(id: &str) -> Result<ObjectId> {
    id.parse()
        .map_err(|_| format!("invalid object ID {id:?}").into())
}

fn object(document: &Document, id: ObjectId) -> Result<&Object> {
    document
        .object(id)
        .ok_or_else(|| format!("unknown object {id}").into())
}

pub(super) fn objects(document: &Document, ids: Option<Vec<String>>) -> Result<Value> {
    let ids = ids
        .map(|ids| {
            ids.iter()
                .map(|id| object_id(id))
                .collect::<Result<Vec<_>>>()
        })
        .transpose()?;
    let selected = document.selected_object_ids().collect::<HashSet<_>>();
    let describe = |object: &Object| {
        let attrs = object.attributes();
        let geometry = object.geometry();
        let kind = match geometry {
            Geometry::Point(_) => "point",
            Geometry::PointCloud(_) => "point_cloud",
            Geometry::Line(_) => "line",
            Geometry::Circle(_) => "circle",
            Geometry::Arc(_) => "arc",
            Geometry::Ellipse(_) => "ellipse",
            Geometry::Polyline(_) => "polyline",
            Geometry::NurbsCurve(_) => "nurbs_curve",
            Geometry::PolyCurve(_) => "polycurve",
            Geometry::NurbsSurface(_) => "nurbs_surface",
            Geometry::Brep(_) => "brep",
            Geometry::Mesh(_) | Geometry::EmptyMesh => "mesh",
            Geometry::SubD(_) => "subd",
            Geometry::BlockInstance(_) => "block_instance",
        };
        json!({
            "id": object.id().to_string(),
            "kind": kind,
            "name": attrs.name(),
            "layer": attrs.layer_id().to_string(),
            "groups": object.group_ids().iter().map(ToString::to_string).collect::<Vec<_>>(),
            "visible": attrs.is_visible(),
            "locked": attrs.is_locked(),
            "selected": selected.contains(&object.id()),
            "bounds": geometry.bounds().map(|bounds| [bounds.min().to_array(), bounds.max().to_array()]),
        })
    };
    let records = if let Some(ids) = ids {
        let all = document
            .objects()
            .map(|object| (object.id(), object))
            .collect::<HashMap<_, _>>();
        ids.into_iter()
            .map(|id| {
                all.get(&id)
                    .map(|object| describe(object))
                    .ok_or_else(|| format!("unknown object {id}"))
            })
            .collect::<std::result::Result<Vec<_>, _>>()?
    } else {
        document.objects().map(describe).collect()
    };
    Ok(json!(records))
}

pub(super) fn selected_ids(document: &Document) -> Vec<String> {
    document
        .selected_object_ids()
        .map(|id| id.to_string())
        .collect()
}

pub(super) fn layers(document: &Document) -> Value {
    json!(
        document
            .layers()
            .map(|layer| {
                let color = layer.color();
                json!({
                    "id": layer.id().to_string(), "name": layer.name(), "number": layer.number(),
                    "color": [color.red, color.green, color.blue],
                    "visible": layer.is_visible(), "locked": layer.is_locked(),
                    "current": layer.id() == document.current_layer_id(),
                })
            })
            .collect::<Vec<_>>()
    )
}

pub(super) fn groups(document: &Document) -> Value {
    json!(
        document
            .groups()
            .map(|group| json!({
                "id": group.id().to_string(),
                "name": group.name(),
                "objects": group.members().map(|id| id.to_string()).collect::<Vec<_>>(),
            }))
            .collect::<Vec<_>>()
    )
}

type Snapshot = HashMap<ObjectId, (usize, u64)>;
pub(super) fn snapshot(document: &Document) -> Snapshot {
    document
        .objects()
        .enumerate()
        .map(|(index, object)| (object.id(), (index, object.geometry_event_serial())))
        .collect()
}

pub(super) fn changes(document: &Document, mut before: Snapshot) -> Value {
    let mut added = Vec::new();
    let mut modified = Vec::new();
    for object in document.objects() {
        let id = object.id();
        match before.remove(&id) {
            None => added.push(id.to_string()),
            Some((_, serial)) if serial != object.geometry_event_serial() => {
                modified.push(id.to_string())
            }
            Some(_) => {}
        }
    }
    let mut deleted = before
        .into_iter()
        .map(|(id, (index, _))| (index, id.to_string()))
        .collect::<Vec<_>>();
    deleted.sort_unstable_by_key(|(index, _)| *index);
    json!({
        "added": added,
        "modified": modified,
        "deleted": deleted.into_iter().map(|(_, id)| id).collect::<Vec<_>>(),
    })
}

fn component_kind(name: &str) -> Result<ComponentSelectionKind> {
    use ComponentSelectionKind::*;
    Ok(match name {
        "brep_edge" => BrepEdge,
        "brep_face" => BrepFace,
        "mesh_edge" => MeshEdge,
        "mesh_face" => MeshFace,
        "mesh_vertex" => MeshVertex,
        "subd_edge" => SubDEdge,
        "subd_face" => SubDFace,
        "subd_vertex" => SubDVertex,
        _ => return Err(format!("unknown component kind {name:?}").into()),
    })
}

fn component_name(kind: ComponentSelectionKind) -> &'static str {
    use ComponentSelectionKind::*;
    match kind {
        BrepEdge => "brep_edge",
        BrepFace => "brep_face",
        MeshEdge => "mesh_edge",
        MeshFace => "mesh_face",
        MeshVertex => "mesh_vertex",
        SubDEdge => "subd_edge",
        SubDFace => "subd_face",
        SubDVertex => "subd_vertex",
    }
}

pub(super) fn components(document: &Document, id: String) -> Result<Value> {
    fn describe_brep(brep: &viboceros_geometry::Brep, values: &mut Vec<Value>) {
        for (index, edge) in brep.edges().iter().enumerate() {
            let points = edge
                .vertices()
                .map(|vertex| brep.vertices()[vertex].point().to_array());
            values.push(json!({"kind": "brep_edge", "index": index, "points": points}));
        }
        for (index, face) in brep.faces().iter().enumerate() {
            let loops = face
                .loops()
                .iter()
                .map(|boundary| {
                    boundary
                        .trims()
                        .iter()
                        .filter_map(|trim| trim.edge())
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>();
            values.push(json!({"kind": "brep_face", "index": index, "loops": loops}));
        }
    }
    let source = object(document, object_id(&id)?)?;
    let mut values = Vec::new();
    match source.geometry() {
        Geometry::Brep(brep) => describe_brep(brep, &mut values),
        Geometry::NurbsSurface(surface) => {
            let brep =
                viboceros_geometry::Brep::try_surface_face(surface.clone(), document.tolerance())?;
            describe_brep(&brep, &mut values);
        }
        Geometry::Mesh(mesh) => {
            for (index, edge) in mesh.topology_edge_points().into_iter().enumerate() {
                values.push(json!({"kind": "mesh_edge", "index": index, "points": edge.map(Point3::to_array)}));
            }
            for (index, point) in mesh.topology_vertex_points().into_iter().enumerate() {
                values.push(
                    json!({"kind": "mesh_vertex", "index": index, "points": [point.to_array()]}),
                );
            }
            for (index, face) in mesh.faces().iter().enumerate() {
                let polygon = face
                    .indices()
                    .iter()
                    .map(|&vertex| mesh.vertices()[vertex as usize].to_array())
                    .collect::<Vec<_>>();
                values.push(json!({"kind": "mesh_face", "index": index, "points": polygon}));
            }
        }
        Geometry::SubD(subd) => {
            let points = subd.control_points();
            for (index, edge) in subd.edges().iter().enumerate() {
                values.push(json!({"kind": "subd_edge", "index": index, "points": edge.map(|vertex| points[vertex as usize].to_array())}));
            }
            for (index, point) in points.iter().enumerate() {
                values.push(
                    json!({"kind": "subd_vertex", "index": index, "points": [point.to_array()]}),
                );
            }
            for (index, face) in subd.faces().iter().enumerate() {
                let polygon = face
                    .iter()
                    .map(|&vertex| points[vertex as usize].to_array())
                    .collect::<Vec<_>>();
                values.push(json!({"kind": "subd_face", "index": index, "points": polygon}));
            }
        }
        _ => {}
    }
    for value in &mut values {
        value["object"] = json!(id);
    }
    Ok(json!(values))
}

pub(super) fn grips(document: &Document, id: String) -> Result<Value> {
    let object_id = object_id(&id)?;
    object(document, object_id)?;
    let hidden = document
        .hidden_control_point_indices(object_id)
        .into_iter()
        .flatten()
        .collect::<HashSet<_>>();
    Ok(json!(
        document
            .all_control_points()
            .filter(|(grip, _, _)| grip.object == object_id)
            .map(|(grip, point, selected)| json!({
                "object": id, "index": grip.index, "point": point.to_array(), "selected": selected,
                "hidden": hidden.contains(&grip.index),
            }))
            .collect::<Vec<_>>()
    ))
}

impl Session {
    pub(super) fn selection(&mut self) -> Value {
        let doc = &self.app.document;
        let components = self.app.component_selection.valid_picks(doc);
        json!({
            "objects": selected_ids(doc),
            "components": components.iter().map(|c| json!({
                "object": c.object.to_string(), "kind": component_name(c.kind), "index": c.index,
            })).collect::<Vec<_>>(),
            "grips": doc.selected_logical_control_points().map(|(g, _)| json!({
                "object": g.object.to_string(), "index": g.index,
            })).collect::<Vec<_>>(),
        })
    }

    pub(super) fn select(&mut self, ids: Vec<String>) -> Result<()> {
        if self.pending() {
            return Err("finish the active getter or use an in-command pick".into());
        }
        let ids = ids
            .iter()
            .map(|id| object_id(id))
            .collect::<Result<Vec<_>>>()?;
        self.app.document.set_transient_selection(ids)?;
        self.app.document.clear_control_point_selection();
        self.app.component_selection.clear();
        Ok(())
    }

    pub(super) fn select_components(&mut self, components: Vec<ComponentRef>) -> Result<()> {
        if self.pending() {
            return Err("finish the active getter or use a viewport pick".into());
        }
        let picks = components
            .into_iter()
            .map(ComponentRef::pick)
            .collect::<Result<Vec<_>>>()?;
        self.app
            .component_selection
            .replace(&self.app.document, picks)?;
        self.app.document.clear_selection();
        Ok(())
    }

    pub(super) fn select_grips(&mut self, grips: Vec<GripRef>) -> Result<()> {
        if self.pending() {
            return Err("finish the active getter or use a viewport pick".into());
        }
        let picks = grips
            .into_iter()
            .map(GripRef::pick)
            .collect::<Result<Vec<_>>>()?;
        self.app
            .document
            .select_control_points(picks, SelectionMode::Replace)?;
        self.app.component_selection.clear();
        Ok(())
    }
}
