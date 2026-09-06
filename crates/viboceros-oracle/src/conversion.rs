//! Shared document/attribute records for geometry conversion commands.
use super::*;
use crate::object_source::ObjectSource;

#[cfg(test)]
mod tests;

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct ConversionFixture {
    pub sources: Vec<ObjectSource>,
    pub selected: Option<Vec<usize>>,
    pub delete_input: Option<bool>,
}

pub(super) fn run(f: &ConversionFixture, tolerance: Tolerance) -> Result<(Value, u64), ProbeError> {
    run_command(
        f,
        tolerance,
        &CommandRegistry::with_builtins(),
        &format!("ConvertToBeziers{}", delete_option(f.delete_input)),
        false,
    )
}

pub(super) fn delete_option(value: Option<bool>) -> &'static str {
    match value {
        Some(true) => " DeleteInput=Yes",
        Some(false) => " DeleteInput=No",
        None => "",
    }
}

pub(super) fn run_command(
    f: &ConversionFixture,
    tolerance: Tolerance,
    registry: &CommandRegistry,
    command: &str,
    undo_after: bool,
) -> Result<(Value, u64), ProbeError> {
    let invalid = || ProbeError::FixtureInvariant("invalid conversion fixture");
    if f.sources.is_empty() || f.sources.len() > 16 {
        return Err(invalid());
    }
    let selected = f
        .selected
        .clone()
        .unwrap_or_else(|| (0..f.sources.len()).collect());
    if selected.is_empty()
        || selected.iter().any(|i| *i >= f.sources.len())
        || selected.iter().collect::<BTreeSet<_>>().len() != selected.len()
    {
        return Err(invalid());
    }
    let mut document = Document::new(tolerance);
    let source_layer = document.add_layer("Source", ColorRgb::BLACK)?;
    let current_layer = document.add_layer("Current", ColorRgb::BLACK)?;
    document.set_current_layer(current_layer)?;
    let mut ids = Vec::new();
    for (i, source) in f.sources.iter().enumerate() {
        ids.push(
            document.add_geometry_with_attributes(
                source.geometry(tolerance)?,
                ObjectAttributes::on_layer(source_layer)
                    .with_name(format!("source-{i}"))
                    .with_object_color(ColorRgb::new(11 + i as u8, 22, 33)),
            )?,
        );
    }
    for (i, members) in [ids.clone(), vec![ids[0]], vec![]].into_iter().enumerate() {
        let group = document.add_empty_group(Some(format!("Group-{i}")))?;
        document.add_group_members(group, members)?;
    }
    for index in selected {
        document.select_objects_direct([ids[index]], SelectionMode::Add)?;
    }
    let inspect_sources = matches!(
        command.split_whitespace().next(),
        Some("ToNURBS" | "MeshToNURB")
    );
    let before = record(
        &document,
        &ids,
        source_layer,
        current_layer,
        inspect_sources,
    )?;
    registry.execute(&mut document, command)?;
    let after = record(
        &document,
        &ids,
        source_layer,
        current_layer,
        inspect_sources,
    )?;
    if undo_after {
        if before == after {
            return Err(ProbeError::FixtureInvariant(
                "cannot undo a no-op conversion",
            ));
        }
        registry.execute(&mut document, "Undo")?;
    }
    Ok((json!({"before":before,"after":after}), 0))
}

fn record(
    document: &Document,
    ids: &[ObjectId],
    source_layer: LayerId,
    current_layer: LayerId,
    inspect_sources: bool,
) -> Result<Value, ProbeError> {
    let mut records = Vec::new();
    for object in document.objects() {
        let original = ids.iter().position(|id| *id == object.id());
        let geometry = object.geometry();
        let (kind, definition) = match geometry {
            Geometry::Point(_) => ("point", Value::Null),
            Geometry::PointCloud(_) => ("point_cloud", Value::Null),
            Geometry::Mesh(_) => ("mesh", Value::Null),
            Geometry::Brep(brep) if inspect_sources => (
                "brep",
                json!({
                    "topology": mesh_to_nurb_brep_value(brep)?,
                    "surfaces": brep.faces().iter().map(|f| surface_definition(f.surface())).collect::<Vec<_>>()
                }),
            ),
            Geometry::Brep(_) if original.is_some() => ("brep", Value::Null),
            Geometry::NurbsSurface(surface) => (
                "surface",
                if original.is_none() || inspect_sources {
                    surface_definition(surface)
                } else {
                    Value::Null
                },
            ),
            _ => (
                "curve",
                if original.is_none() || inspect_sources {
                    curve_definition(
                        &geometry
                            .curve_ref()
                            .ok_or(ProbeError::FixtureInvariant("unexpected conversion output"))?
                            .to_nurbs()?,
                    )
                } else {
                    Value::Null
                },
            ),
        };
        let (domain, points) = crate::distribute::sample(geometry)?;
        let attrs = object.attributes();
        let layer = if attrs.layer_id() == source_layer {
            "Source"
        } else if attrs.layer_id() == current_layer {
            "Current"
        } else {
            "Unexpected"
        };
        let color = attrs.object_color();
        let color_source = match attrs.color_source() {
            ObjectColorSource::Layer => "ColorFromLayer",
            ObjectColorSource::Object => "ColorFromObject",
            ObjectColorSource::Material => "ColorFromMaterial",
            ObjectColorSource::Parent => "ColorFromParent",
        };
        let groups = object
            .group_ids()
            .iter()
            .map(|id| document.group(*id).unwrap().name().unwrap())
            .collect::<Vec<_>>();
        let mut value = json!({"original":original,"kind":kind,"domain":domain,"points":points,"definition":definition,
            "name":attrs.name(),"layer":layer,"color":[color.red,color.green,color.blue],"color_source":color_source,"groups":groups,"selected":document.is_selected(object.id())});
        if inspect_sources {
            value["representation"] = json!(match geometry {
                Geometry::Line(_) => "LineCurve",
                Geometry::Arc(_) | Geometry::Circle(_) => "ArcCurve",
                Geometry::Polyline(_) => "PolylineCurve",
                Geometry::PolyCurve(_) => "PolyCurve",
                Geometry::NurbsCurve(_) | Geometry::Ellipse(_) => "NurbsCurve",
                _ => kind,
            });
        }
        let key = (
            original.is_none(),
            original.unwrap_or(0),
            kind,
            points
                .iter()
                .flatten()
                .map(|x| (x * 1e8).round())
                .collect::<Vec<_>>(),
        );
        records.push((key, object.id(), value));
    }
    records.sort_by(|a, b| a.0.partial_cmp(&b.0).expect("finite fixture coordinates"));
    let index = records
        .iter()
        .enumerate()
        .map(|(i, r)| (r.1, i))
        .collect::<BTreeMap<_, _>>();
    let creation_order = document
        .objects()
        .map(|o| index[&o.id()])
        .collect::<Vec<_>>();
    let mut groups = document
        .groups()
        .map(|g| {
            let mut members = g.members().map(|id| index[&id]).collect::<Vec<_>>();
            members.sort_unstable();
            (g.name().unwrap(), members)
        })
        .collect::<Vec<_>>();
    groups.sort_by_key(|g| g.0);
    Ok(
        json!({"objects":records.into_iter().map(|(_,_,v)|v).collect::<Vec<_>>(),"creation_order":creation_order,
        "groups":groups.into_iter().map(|(name,members)|json!({"name":name,"members":members})).collect::<Vec<_>>()}),
    )
}

fn surface_definition(surface: &NurbsSurface) -> Value {
    let mut value = nurbs_surface_definition_value(surface);
    // OpenNURBS omits the two superfluous end knots. The Rhino worker pads
    // these nonexistent slots with their neighbors; use that same codec here,
    // without changing model knots or dropping any actual Rhino knot value.
    for key in ["knots_u", "knots_v"] {
        let knots = value[key].as_array_mut().expect("surface knot array");
        let last = knots.len() - 1;
        knots[0] = knots[1].clone();
        knots[last] = knots[last - 1].clone();
    }
    value
}

fn curve_definition(curve: &NurbsCurve) -> Value {
    let mut value = nurbs_curve_definition_value(curve);
    // Same omitted-outer-knot convention as the surface codec above.
    let knots = value["knots"].as_array_mut().expect("curve knot array");
    let last = knots.len() - 1;
    knots[0] = knots[1].clone();
    knots[last] = knots[last - 1].clone();
    value
}
