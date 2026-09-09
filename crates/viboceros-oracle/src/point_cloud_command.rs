//! PointCloud conversion: source lifetime, attributes, selection, and point order.
use super::*;
use crate::object_source::ObjectSource;
use viboceros_document::ObjectColorSource;

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct PointCloudFixture {
    pub sources: Vec<ObjectSource>,
    pub selected: Option<Vec<usize>>,
    #[serde(default)]
    pub postselect: bool,
}

pub(super) fn run(f: &PointCloudFixture, tolerance: Tolerance) -> Result<(Value, u64), ProbeError> {
    if f.sources.is_empty() || f.sources.len() > 16 {
        return Err(ProbeError::FixtureInvariant(
            "expected 1 to 16 cloud sources",
        ));
    }
    let mut document = Document::new(tolerance);
    let source_layer = document.current_layer_id();
    let current_layer = document.add_layer("Current", ColorRgb::BLACK)?;
    document.set_current_layer(current_layer)?;
    let mut ids = Vec::new();
    for (i, source) in f.sources.iter().enumerate() {
        ids.push(document.add_geometry_with_attributes(
            source.geometry(tolerance)?,
            ObjectAttributes::on_layer(source_layer).with_name(format!("source-{i}")),
        )?);
    }
    let selected = f
        .selected
        .clone()
        .unwrap_or_else(|| (0..ids.len()).collect());
    if selected.is_empty()
        || selected.iter().any(|i| *i >= ids.len())
        || selected.iter().collect::<BTreeSet<_>>().len() != selected.len()
    {
        return Err(ProbeError::FixtureInvariant("invalid cloud selection"));
    }
    let selected_ids = selected
        .iter()
        .map(|i| ids[*i])
        .filter(|id| {
            !f.postselect
                || viboceros_command::ObjectSelectionFilter::PointCloudSources
                    .accepts(document.object(*id).unwrap().geometry())
        })
        .collect::<Vec<_>>();
    // Individual selection actions retain pick order; a batch is intentionally
    // normalized to document order by the document API.
    for id in selected_ids {
        document.select_objects_direct([id], SelectionMode::Add)?;
    }
    let registry = CommandRegistry::with_builtins();
    if f.postselect {
        registry.execute_postselected(
            &mut document,
            "PointCloud UsePointColors=No",
            viboceros_command::CommandContext::default(),
        )?;
    } else {
        registry.execute(&mut document, "PointCloud")?;
    }
    let mut objects = Vec::new();
    for object in document.objects() {
        let (kind, points): (&str, Vec<Point3>) = match object.geometry() {
            Geometry::Point(p) => ("point", vec![*p]),
            Geometry::PointCloud(p) => ("point_cloud", p.points().to_vec()),
            Geometry::Mesh(m) => ("mesh", m.vertices().to_vec()),
            _ => ("other", vec![]),
        };
        let attributes = object.attributes();
        let color_source = match attributes.color_source() {
            ObjectColorSource::Layer => "ColorFromLayer",
            ObjectColorSource::Object => "ColorFromObject",
            ObjectColorSource::Material => "ColorFromMaterial",
            ObjectColorSource::Parent => "ColorFromParent",
        };
        objects.push(
            json!({"original": ids.iter().position(|id| *id == object.id()),
            "kind": kind, "points": points.iter().map(|p| p.to_array()).collect::<Vec<_>>(),
            "name": attributes.name(), "selected": document.is_selected(object.id()),
            "layer": if attributes.layer_id() == source_layer {"Source"} else {"Current"},
            "color_source": color_source, "has_colors": false}),
        );
    }
    objects.sort_by_key(|o| (o["original"].is_null(), o["original"].as_u64().unwrap_or(0)));
    Ok((json!({"objects": objects}), 0))
}

#[cfg(test)]
mod tests {
    #[test]
    fn creation_fixture_matches_rhino_geometry_and_document_state() {
        let request: crate::ProbeRequest = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/point_cloud_command.json"
        ))
        .unwrap();
        let actual = crate::run_request(&request).unwrap();
        let expected: serde_json::Value = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/observations/point_cloud_command.json"
        ))
        .unwrap();
        assert_eq!(actual.results.len(), 8);
        assert_eq!(expected["results"].as_array().unwrap().len(), 8);
        for result in actual.results {
            let reference = expected["results"]
                .as_array()
                .unwrap()
                .iter()
                .find(|r| r["id"] == result.id)
                .unwrap();
            assert_eq!(result.value, reference["value"], "{}", result.id);
        }
    }
}
