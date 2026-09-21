//! Cap on shared source topology. Compare physical boundaries, not UV frames.
use super::*;
use crate::brep_source::{BrepCommandSource, reorder_edges, write_shared_artifact};
#[cfg(test)]
mod tests;

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct CapFixture {
    source: BrepCommandSource,
    #[serde(default)]
    preselect: bool,
    #[serde(default)]
    reversed: bool,
    keep_faces: Option<Vec<usize>>,
    edge_order: Option<Vec<usize>>,
    artifact_path: Option<String>,
}

pub(super) fn run(f: &CapFixture, tolerance: Tolerance) -> Result<(Value, u64), ProbeError> {
    let geometry = f.source.geometry(tolerance)?;
    let mut brep = match geometry {
        Geometry::Brep(brep) => brep,
        Geometry::NurbsSurface(surface) => Brep::try_surface_face(surface, tolerance)?,
        _ => {
            return Err(ProbeError::FixtureInvariant(
                "Cap probe requires a surface or B-rep",
            ));
        }
    };
    if let Some(faces) = &f.keep_faces {
        brep = brep.sub_brep(faces, tolerance)?;
    }
    if let Some(order) = &f.edge_order {
        brep = reorder_edges(&brep, order, tolerance)?;
    }
    if f.reversed {
        brep = brep.reversed();
    }
    let before = geometry_record(&brep, tolerance)?;
    let geometry = Geometry::Brep(brep);
    if let Some(path) = &f.artifact_path {
        write_shared_artifact(&geometry, path, tolerance)?;
    }
    let mut document = Document::new(tolerance);
    let source_layer = document.add_layer("Source", ColorRgb::BLACK)?;
    let current_layer = document.add_layer("Current", ColorRgb::BLACK)?;
    document.set_current_layer(current_layer)?;
    let source = document.add_geometry_with_attributes(
        geometry,
        ObjectAttributes::on_layer(source_layer)
            .with_name("Source")
            .with_object_color(ColorRgb::new(11, 22, 33)),
    )?;
    document.add_group(Some("Source group".into()), [source])?;
    document.select_objects_direct([source], SelectionMode::Replace)?;
    let registry = CommandRegistry::with_builtins();
    if f.preselect {
        registry.execute(&mut document, "Cap")?;
    } else {
        registry.execute_postselected(&mut document, "Cap", Default::default())?;
    }
    let objects = document.objects().map(|o| {
        let Geometry::Brep(brep) = o.geometry() else { return Err(ProbeError::FixtureInvariant("Cap lost B-rep")); };
        let attrs = o.attributes(); let color = attrs.object_color();
        Ok(json!({"source":o.id()==source, "name":attrs.name(), "selected":document.is_selected(o.id()),
            "layer":if attrs.layer_id()==source_layer {"Source"} else {"Unexpected"},
            "color":[color.red,color.green,color.blue], "color_source":format!("ColorFrom{:?}",attrs.color_source()),
            "group_count":o.group_ids().len(), "geometry":geometry_record(brep,tolerance)?}))
    }).collect::<Result<Vec<_>,ProbeError>>()?;
    Ok((
        json!({"succeeded":true,"input":before,"objects":objects}),
        0,
    ))
}

fn geometry_record(brep: &Brep, tolerance: Tolerance) -> Result<Value, ProbeError> {
    // Integrals are witnesses, not modeling tolerances. Resolve them more
    // accurately than the comparison epsilon without changing source geometry.
    let measure = Tolerance::try_new(
        tolerance.absolute().min(1e-12),
        tolerance.relative().min(1e-13),
        tolerance.angular(),
    )?;
    let mut faces = Vec::new();
    for (index, face) in brep.faces().iter().enumerate() {
        let mut loops = face
            .loops()
            .iter()
            .map(|l| {
                let mut edges = l
                    .trims()
                    .iter()
                    .map(|t| (t.edge(), t.is_reversed_3d() ^ face.is_reversed()))
                    .collect::<Vec<_>>();
                edges.sort();
                (format!("{:?}", l.loop_type()), edges)
            })
            .collect::<Vec<_>>();
        loops.sort();
        // Ordered source edge IDs identify a face independently of generated
        // planar parameterization, loop start, and cap-face insertion order.
        faces.push((loops, brep.sub_brep(&[index], tolerance)?.area(measure)?));
    }
    faces.sort_by(|a, b| a.0.cmp(&b.0));
    let counts = brep.edge_use_counts();
    let edges = brep.edges().iter().enumerate().map(|(index,e)| {
        Ok(json!({"vertices":e.vertices(),"uses":counts[index],
            "curve":crate::curve_interchange::curve_record(viboceros_geometry::CurveRef::NurbsCurve(e.curve()))?}))
    }).collect::<Result<Vec<_>,ProbeError>>()?;
    let volume = if brep.is_solid() {
        Some(brep.signed_volume(measure)?)
    } else {
        None
    };
    Ok(
        json!({"vertices":brep.vertices().iter().map(|v|v.point().to_array()).collect::<Vec<_>>(),
        "edges":edges,"faces":faces,"solid":brep.is_solid(),"manifold":brep.is_manifold(),"volume":volume}),
    )
}
