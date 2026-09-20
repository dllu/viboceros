//! Shared owned sources, groups, selection and recording for layout probes.
use super::*;
use crate::object_source::ObjectSource;

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct ObjectLayoutFixture {
    pub origin: [f64; 3],
    pub x_axis: [f64; 3],
    pub y_axis: [f64; 3],
    pub sources: Vec<ObjectSource>,
    #[serde(default)]
    pub groups: Vec<Vec<usize>>,
    pub selected: Option<Vec<usize>>,
    #[serde(default = "preselect_default")]
    pub preselect: bool,
}

fn preselect_default() -> bool {
    true
}

pub(super) fn run(
    f: &ObjectLayoutFixture,
    tolerance: Tolerance,
    minimum_selection: usize,
    execute: impl FnOnce(&mut Document, Frame3) -> Result<bool, ProbeError>,
) -> Result<(Value, u64), ProbeError> {
    let invalid = || ProbeError::FixtureInvariant("invalid object layout fixture");
    if f.sources.is_empty() || f.sources.len() > 32 {
        return Err(invalid());
    }
    let plane = Frame3::try_from_directions(
        Point3::try_from(f.origin)?,
        Vector3::try_from(f.x_axis)?,
        Vector3::try_from(f.y_axis)?,
        tolerance,
    )?;
    let selected = f
        .selected
        .clone()
        .unwrap_or_else(|| (0..f.sources.len()).collect());
    let valid_indices = |indices: &[usize]| {
        indices.iter().all(|i| *i < f.sources.len())
            && indices.iter().collect::<BTreeSet<_>>().len() == indices.len()
    };
    if selected.len() < minimum_selection
        || !valid_indices(&selected)
        || f.groups.iter().any(|g| g.is_empty() || !valid_indices(g))
    {
        return Err(invalid());
    }
    let mut document = Document::new(tolerance);
    let mut ids = Vec::new();
    for (i, source) in f.sources.iter().enumerate() {
        ids.push(document.add_geometry_with_attributes(
            source.geometry(tolerance)?,
            ObjectAttributes::on_layer(document.current_layer_id()).with_name(i.to_string()),
        )?);
    }
    for (i, group) in f.groups.iter().enumerate() {
        document.add_group(Some(format!("sources-{i}")), group.iter().map(|i| ids[*i]))?;
    }
    // Match the worker's individual selection actions, including their order.
    for index in &selected {
        // Both preselection and the worker's SelID prompt actions select only
        // the named objects, unlike a grouped viewport click.
        document.select_objects_direct([ids[*index]], SelectionMode::Add)?;
    }
    let succeeded = execute(&mut document, plane)?;
    let mut records = Vec::new();
    for object in document.objects() {
        let source = object
            .attributes()
            .name()
            .ok_or_else(invalid)?
            .parse::<usize>()
            .map_err(|_| invalid())?;
        let (domain, points) = sample(object.geometry())?;
        records.push(json!({"source":source,"retained":object.id()==ids[source],"selected":document.is_selected(object.id()),
            "current_layer":object.attributes().layer_id()==document.current_layer_id(),"domain":domain,"points":points}));
    }
    records.sort_by_key(|r| r["source"].as_u64().unwrap());
    let mut groups = document
        .groups()
        .map(|g| {
            let mut members = g
                .members()
                .map(|id| {
                    document
                        .object(id)
                        .unwrap()
                        .attributes()
                        .name()
                        .unwrap()
                        .parse::<usize>()
                        .unwrap()
                })
                .collect::<Vec<_>>();
            members.sort_unstable();
            members
        })
        .collect::<Vec<_>>();
    groups.sort();
    Ok((
        json!({"succeeded":succeeded,"objects":records,"groups":groups}),
        0,
    ))
}

pub(super) fn sample(geometry: &Geometry) -> Result<(Value, Vec<[f64; 3]>), GeometryError> {
    let points = match geometry {
        Geometry::Point(p) => vec![p.to_array()],
        Geometry::PointCloud(c) => c.points().iter().map(|p| p.to_array()).collect(),
        Geometry::Mesh(m) => m.vertices().iter().map(|p| p.to_array()).collect(),
        Geometry::Brep(b) => return plane_arrays::brep_record(b),
        Geometry::NurbsSurface(s) => {
            let u = s.domain_u();
            let v = s.domain_v();
            let points = (0..=4)
                .flat_map(|j| (0..=4).map(move |i| (i, j)))
                .map(|(i, j)| {
                    s.evaluate(
                        u.start() + (u.end() - u.start()) * f64::from(i) / 4.,
                        v.start() + (v.end() - v.start()) * f64::from(j) / 4.,
                    )
                    .map(|p| p.to_array())
                })
                .collect::<Result<Vec<_>, _>>()?;
            return Ok((
                json!([[*u.start(), *u.end()], [*v.start(), *v.end()]]),
                points,
            ));
        }
        _ => {
            let curve = geometry.curve_ref().expect("remaining geometry is a curve");
            let domain = curve.domain();
            let (a, b) = (*domain.start(), *domain.end());
            return Ok((
                json!([a, b]),
                (0..=32)
                    .map(|i| {
                        curve
                            .evaluate(a + (b - a) * f64::from(i) / 32.)
                            .map(|p| p.to_array())
                    })
                    .collect::<Result<Vec<_>, _>>()?,
            ));
        }
    };
    Ok((Value::Null, points))
}
