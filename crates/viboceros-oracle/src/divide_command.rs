//! Actual Divide outputs, domains, source attributes and per-command groups.
use super::*;
use crate::curve_join_close::CurveInput;
use viboceros_geometry::Real;

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Fixture {
    pub sources: Vec<CurveInput>,
    pub mode: String,
    pub value: Real,
    #[serde(default)]
    pub mark_ends: bool,
    #[serde(default)]
    pub split: bool,
    #[serde(default)]
    pub delete_remainder: bool,
    #[serde(default)]
    pub group: bool,
}
pub(super) fn run(f: &Fixture, tolerance: Tolerance) -> Result<(Value, u64), ProbeError> {
    if f.sources.is_empty()
        || f.sources.len() > 64
        || !matches!(f.mode.as_str(), "count" | "length" | "chord")
    {
        return Err(ProbeError::FixtureInvariant("invalid Divide fixture"));
    }
    let mut doc = Document::new(tolerance);
    let current = doc.current_layer_id();
    let mut ids = Vec::new();
    for (i, source) in f.sources.iter().enumerate() {
        let layer = doc.add_layer(
            format!("Source{i}"),
            viboceros_document::ColorRgb::new(0, 0, 0),
        )?;
        let attrs = ObjectAttributes::on_layer(layer)
            .with_name(format!("Source{i}"))
            .with_object_color(viboceros_document::ColorRgb::new(20 + i as u8, 40, 60));
        let curve = source.geometry()?;
        // The shared native fixture builder constructs circles as NURBS.
        let geometry = if matches!(source, CurveInput::Circle { .. }) {
            Geometry::NurbsCurve(curve.as_ref().to_nurbs()?)
        } else {
            curve.into()
        };
        ids.push(doc.add_geometry_with_attributes(geometry, attrs)?);
    }
    doc.select_objects_direct(ids.iter().copied(), SelectionMode::Replace)?;
    let mode = match f.mode.as_str() {
        "count" => "",
        "length" => "Length ",
        _ => "EqualChordLength ",
    };
    let yes = |b| if b { "Yes" } else { "No" };
    let command = format!(
        "Divide {mode}{} MarkEnds={} Split={} DeleteRemainder={} GroupOutput={}",
        f.value,
        yes(f.mark_ends),
        yes(f.split),
        yes(f.delete_remainder),
        yes(f.group)
    );
    let succeeded = CommandRegistry::with_builtins()
        .execute(&mut doc, &command)
        .is_ok();
    let mut outputs = Vec::new();
    let mut group_map = BTreeMap::new();
    for obj in doc.objects().filter(|o| !ids.contains(&o.id())) {
        let a = obj.attributes();
        let mut groups = Vec::new();
        for id in obj.group_ids() {
            let n = group_map.len();
            groups.push(*group_map.entry(*id).or_insert(n));
        }
        let (kind, points, domain, closed) = if let Geometry::Point(p) = obj.geometry() {
            ("point", vec![p.to_array()], Value::Null, false)
        } else {
            let curve = obj
                .geometry()
                .curve_ref()
                .ok_or(ProbeError::FixtureInvariant("invalid Divide output"))?;
            let d = curve.domain();
            let mut points = Vec::new();
            for i in 0..=16 {
                let t = if i == 0 {
                    *d.start()
                } else if i == 16 {
                    *d.end()
                } else {
                    *d.start() + (d.end() - d.start()) * i as Real / 16.
                };
                points.push(curve.evaluate(t)?.to_array());
            }
            (
                "curve",
                points,
                json!([d.start(), d.end()]),
                curve.is_closed()?,
            )
        };
        outputs.push(json!({"kind":kind,"points":points,"domain":domain,"closed":closed,"name":a.name().unwrap_or(""),"layer":if a.layer_id()==current{"current"}else{"source"},"color_source":if a.color_source()==viboceros_document::ObjectColorSource::Object{"ColorFromObject"}else{"ColorFromLayer"},"groups":groups}));
    }
    Ok((
        json!({"succeeded":succeeded,"outputs":outputs,"input_count":ids.iter().filter(|id|doc.object(**id).is_some()).count(),"group_count":group_map.len()}),
        0,
    ))
}
