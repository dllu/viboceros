//! Command-level Contour geometry and property records from owned sources.
use super::*;
use crate::object_source::ObjectSource;
use viboceros_command::CommandContext;
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Fixture {
    pub sources: Vec<ObjectSource>,
    pub start: [f64; 3],
    pub end: [f64; 3],
    #[serde(default)]
    pub origin: [f64; 3],
    #[serde(default = "x_axis")]
    pub x_axis: [f64; 3],
    #[serde(default = "y_axis")]
    pub y_axis: [f64; 3],
    pub spacing: f64,
    #[serde(default)]
    pub range: bool,
    #[serde(default = "current")]
    pub properties: String,
    #[serde(default)]
    pub group: bool,
}
fn x_axis() -> [f64; 3] {
    [1., 0., 0.]
}
fn y_axis() -> [f64; 3] {
    [0., 1., 0.]
}
fn current() -> String {
    "current".to_owned()
}
pub(super) fn run(f: &Fixture, tolerance: Tolerance) -> Result<(Value, u64), ProbeError> {
    if f.sources.is_empty()
        || f.sources.len() > 64
        || !matches!(f.properties.as_str(), "current" | "input")
    {
        return Err(ProbeError::FixtureInvariant("invalid Contour fixture"));
    }
    let mut doc = Document::new(tolerance);
    let mut layers = Vec::new();
    let mut ids = Vec::new();
    for (i, source) in f.sources.iter().enumerate() {
        let layer = doc.add_layer(
            format!("Source{i}"),
            viboceros_document::ColorRgb::new(0, 0, 0),
        )?;
        layers.push(layer);
        let attributes = ObjectAttributes::on_layer(layer)
            .with_name(format!("source-{i}"))
            .with_object_color(viboceros_document::ColorRgb::new(
                21 + i as u8,
                43 + i as u8,
                65 + i as u8,
            ));
        ids.push(doc.add_geometry_with_attributes(source.geometry(tolerance)?, attributes)?);
    }
    let layer = doc.add_layer("Current", viboceros_document::ColorRgb::new(0, 0, 0))?;
    doc.set_current_layer(layer)?;
    doc.select_objects_direct(ids.iter().copied(), SelectionMode::Replace)?;
    let frame = Frame3::try_from_directions(
        Point3::try_from(f.origin)?,
        Vector3::try_from(f.x_axis)?,
        Vector3::try_from(f.y_axis)?,
        tolerance,
    )?;
    let point = |p: [f64; 3]| format!("{},{},{}", p[0], p[1], p[2]);
    let command = format!(
        "Contour {} {} {} Range={} AssignProperties={} Output=CurvesOnly GroupObjectsByContourPlane={}",
        point(f.start),
        point(f.end),
        f.spacing,
        if f.range { "Yes" } else { "No" },
        if f.properties == "input" {
            "ByInputObject"
        } else {
            "ByCurrentLayer"
        },
        if f.group { "Yes" } else { "No" }
    );
    let result = CommandRegistry::with_builtins().execute_in_context(
        &mut doc,
        &command,
        CommandContext {
            construction_plane: frame,
        },
    );
    let mut outputs = Vec::new();
    for object in doc.objects().filter(|o| !ids.contains(&o.id())) {
        let attributes = object.attributes();
        let color = attributes.object_color();
        let (kind, points, domain, closed) = if let Geometry::Point(point) = object.geometry() {
            ("point", vec![point.to_array()], Value::Null, false)
        } else {
            let curve = object
                .geometry()
                .curve_ref()
                .ok_or(ProbeError::FixtureInvariant("unsupported Contour output"))?;
            let domain = curve.domain();
            let points = (0..=16)
                .map(|i| {
                    curve
                        .evaluate(if i == 0 {
                            *domain.start()
                        } else if i == 16 {
                            *domain.end()
                        } else {
                            *domain.start() + (*domain.end() - *domain.start()) * i as f64 / 16.
                        })
                        .map(Point3::to_array)
                })
                .collect::<Result<Vec<_>, _>>()?;
            (
                "curve",
                points,
                json!([domain.start(), domain.end()]),
                curve.is_closed()?,
            )
        };
        let color_source = match attributes.color_source() {
            viboceros_document::ObjectColorSource::Layer => "ColorFromLayer",
            viboceros_document::ObjectColorSource::Object => "ColorFromObject",
            viboceros_document::ObjectColorSource::Parent => "ColorFromParent",
            viboceros_document::ObjectColorSource::Material => "ColorFromMaterial",
        };
        outputs.push(json!({"kind":kind,"points":points,"domain":domain,"closed":closed,"name":attributes.name(),"layer":doc.layer(attributes.layer_id()).unwrap().name(),"color":[color.red,color.green,color.blue],"color_source":color_source,"groups":object.group_ids().iter().map(|id|doc.groups().position(|g|g.id()==*id).unwrap()).collect::<Vec<_>>()}));
    }
    Ok((
        json!({"succeeded":result.is_ok(),"outputs":outputs,"input_count":ids.iter().filter(|id|doc.object(**id).is_some()).count()}),
        0,
    ))
}
