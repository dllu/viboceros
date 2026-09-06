//! Actual BoundingBox output, topology, grouping, and selection observations.
use super::*;
use crate::object_source::ObjectSource as Source;
use viboceros_command::CommandContext;
#[cfg(test)]
mod tests;

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct BoundingBoxFixture {
    pub origin: [f64; 3],
    pub x_axis: [f64; 3],
    pub y_axis: [f64; 3],
    pub coordinate_system: CoordinateSystem,
    pub cumulative: bool,
    pub output: Output,
    pub sources: Vec<Source>,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
pub enum CoordinateSystem {
    World,
    CPlane,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
pub enum Output {
    Solids,
    Meshes,
    Curves,
    None,
}

pub(super) fn run(
    f: &BoundingBoxFixture,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    if f.sources.is_empty() || f.sources.len() > 16 {
        return Err(ProbeError::FixtureInvariant(
            "expected 1 to 16 BoundingBox sources",
        ));
    }
    let plane = Frame3::try_from_directions(
        Point3::try_from(f.origin)?,
        Vector3::try_from(f.x_axis)?,
        Vector3::try_from(f.y_axis)?,
        tolerance,
    )?;
    let mut document = Document::new(tolerance);
    let mut ids = Vec::new();
    for (i, source) in f.sources.iter().enumerate() {
        let attributes = ObjectAttributes::on_layer(document.current_layer_id())
            .with_name(format!("bbox-source-{i}"));
        ids.push(document.add_geometry_with_attributes(source.geometry(tolerance)?, attributes)?);
    }
    document.select_objects_direct(ids.iter().copied(), SelectionMode::Replace)?;
    let command = format!(
        "BoundingBox CoordinateSystem={:?} Cumulative={} Output={:?}",
        f.coordinate_system,
        if f.cumulative { "Yes" } else { "No" },
        f.output
    );
    let result = CommandRegistry::with_builtins().execute_in_context(
        &mut document,
        &command,
        CommandContext {
            construction_plane: plane,
        },
    );
    let (succeeded, reported_boxes) = match result {
        Ok(message) => (true, message.matches(" size ").count()),
        Err(CommandError::DegenerateBoundingBox) => (false, 0),
        Err(error) => return Err(error.into()),
    };
    let mut records = Vec::new();
    for object in document.objects().filter(|o| !ids.contains(&o.id())) {
        let (mut record, mut points) = record_geometry(object.geometry())?;
        points.sort_by(|a, b| point_key(a).partial_cmp(&point_key(b)).unwrap());
        record["points"] = json!(points);
        record["selected"] = json!(document.is_selected(object.id()));
        record["current_layer"] =
            json!(object.attributes().layer_id() == document.current_layer_id());
        record["name"] = json!(object.attributes().name());
        let key = (
            record["kind"].as_str().unwrap().to_owned(),
            points.iter().flat_map(point_key).collect::<Vec<_>>(),
        );
        records.push((key, record));
    }
    records.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    let mut groups = document
        .groups()
        .map(|g| g.members().len())
        .collect::<Vec<_>>();
    groups.sort_unstable();
    Ok((
        json!({
            "succeeded": succeeded,
            "reported_boxes": reported_boxes,
            "objects": records.into_iter().map(|(_,r)|r).collect::<Vec<_>>(),
            "group_sizes": groups,
            "sources_retained": ids.iter().enumerate().filter_map(|(i,id)|document.object(*id).is_some().then_some(i)).collect::<Vec<_>>(),
            "selected_sources": ids.iter().enumerate().filter_map(|(i,id)|document.is_selected(*id).then_some(i)).collect::<Vec<_>>(),
        }),
        0,
    ))
}

fn point_key(p: &[f64; 3]) -> [f64; 3] {
    p.map(|v| (v * 1e8).round() / 1e8)
}

fn record_geometry(geometry: &Geometry) -> Result<(Value, Vec<[f64; 3]>), ProbeError> {
    Ok(match geometry {
        Geometry::Brep(b) => (
            json!({"kind":"brep", "faces":b.faces().len(), "closed":b.is_solid()}),
            b.vertices().iter().map(|v| v.point().to_array()).collect(),
        ),
        Geometry::Mesh(m) => {
            let mut sizes = m
                .faces()
                .iter()
                .map(|f| match f {
                    MeshFace::Triangle(_) => 3,
                    MeshFace::Quad(_) => 4,
                })
                .collect::<Vec<_>>();
            sizes.sort_unstable();
            (
                json!({"kind":"mesh", "face_sizes":sizes, "closed":m.topology().is_closed()}),
                m.vertices().iter().map(|p| p.to_array()).collect(),
            )
        }
        Geometry::Polyline(p) => {
            let mut points = p
                .vertices()
                .iter()
                .map(|p| p.to_array())
                .collect::<Vec<_>>();
            points.sort_by(|a, b| a.partial_cmp(b).unwrap());
            points.dedup();
            (
                json!({"kind":"curve", "closed":p.is_closed(), "degree":1}),
                points,
            )
        }
        _ => {
            return Err(ProbeError::FixtureInvariant(
                "unexpected BoundingBox output geometry",
            ));
        }
    })
}
