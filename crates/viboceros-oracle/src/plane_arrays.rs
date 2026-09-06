//! Actual array commands on arbitrary planes, including curved bound witnesses.
#[cfg(test)]
mod tests;
use super::*;
use crate::curve_join_close::CurveInput;
use viboceros_command::CommandContext;

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct PlaneArrayFixture {
    pub origin: [f64; 3],
    pub x_axis: [f64; 3],
    pub y_axis: [f64; 3],
    pub sources: Vec<ArraySource>,
    #[serde(default)]
    pub groups: Option<Vec<Vec<usize>>>,
    /// Explicit output count for zero-spacing cells omitted by the command.
    #[serde(default)]
    pub expected_object_count: Option<usize>,
    #[serde(flatten)]
    pub array: ArrayDefinition,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
pub enum ArrayMode {
    UnitCell,
    Fill,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum ArraySource {
    Surface(SurfaceSource),
    Curve(CurveInput),
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SurfaceSource {
    Surface {
        #[serde(flatten)]
        surface: NurbsSurfaceDefinition,
    },
}

impl ArraySource {
    fn geometry(&self) -> Result<Geometry, GeometryError> {
        match self {
            Self::Surface(SurfaceSource::Surface { surface }) => {
                nurbs_surface_from_definition(surface).map(Geometry::NurbsSurface)
            }
            Self::Curve(curve) => curve.geometry().map(Geometry::from),
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(tag = "command")]
pub enum ArrayDefinition {
    Array {
        counts: [usize; 3],
        distances: [f64; 3],
        mode: ArrayMode,
    },
    ArrayLinear {
        item_count: usize,
        references: [[f64; 3]; 2],
    },
    ArrayPolar {
        item_count: usize,
        center: [f64; 3],
        angle: f64,
        rotate: bool,
        z_offset: f64,
    },
}

pub(super) fn run(f: &PlaneArrayFixture, tolerance: Tolerance) -> Result<(Value, u64), ProbeError> {
    let error = || ProbeError::FixtureInvariant("invalid plane array fixture");
    if f.sources.is_empty() || f.sources.len() > 16 {
        return Err(error());
    }
    let plane = Frame3::try_from_directions(
        Point3::try_from(f.origin)?,
        Vector3::try_from(f.x_axis)?,
        Vector3::try_from(f.y_axis)?,
        tolerance,
    )?;
    let point = |p: &[f64; 3]| format!("{},{},{}", p[0], p[1], p[2]);
    let (count, command) = match &f.array {
        ArrayDefinition::Array {
            counts,
            distances,
            mode,
        } => {
            if counts.iter().any(|n| !(1..=64).contains(n)) {
                return Err(error());
            }
            (
                counts.iter().product(),
                format!(
                    "Array {} {} {} {} {} {} Mode={}",
                    counts[0],
                    counts[1],
                    counts[2],
                    distances[0],
                    distances[1],
                    distances[2],
                    if *mode == ArrayMode::Fill {
                        "Fill"
                    } else {
                        "UnitCell"
                    }
                ),
            )
        }
        ArrayDefinition::ArrayLinear {
            item_count,
            references,
        } => (
            *item_count,
            format!(
                "ArrayLinear {item_count} {} {}",
                point(&references[0]),
                point(&references[1])
            ),
        ),
        ArrayDefinition::ArrayPolar {
            item_count,
            center,
            angle,
            rotate,
            z_offset,
        } => (
            *item_count,
            format!(
                "ArrayPolar {item_count} {} {angle} Rotate={} ZOffset={z_offset}",
                point(center),
                if *rotate { "Yes" } else { "No" }
            ),
        ),
    };
    if !(2..=256).contains(&count) {
        return Err(error());
    }
    let mut document = Document::new(tolerance);
    let mut ids = Vec::new();
    for (index, source) in f.sources.iter().enumerate() {
        let attributes =
            ObjectAttributes::on_layer(document.current_layer_id()).with_name(index.to_string());
        ids.push(document.add_geometry_with_attributes(source.geometry()?, attributes)?);
    }
    let groups = f
        .groups
        .clone()
        .unwrap_or_else(|| vec![(0..ids.len()).collect()]);
    for (index, group) in groups.iter().enumerate() {
        if group.is_empty()
            || group.iter().any(|i| *i >= ids.len())
            || group.iter().collect::<BTreeSet<_>>().len() != group.len()
        {
            return Err(error());
        }
        document.add_group(
            Some(format!("array-sources-{index}")),
            group.iter().map(|i| ids[*i]),
        )?;
    }
    document.select_objects_direct(ids.iter().copied(), SelectionMode::Replace)?;
    CommandRegistry::with_builtins().execute_in_context(
        &mut document,
        &command,
        CommandContext {
            construction_plane: plane,
        },
    )?;
    if document.objects().len() != f.expected_object_count.unwrap_or(count * ids.len()) {
        return Err(error());
    }
    let mut records = Vec::new();
    for object in document.objects() {
        let source = object
            .attributes()
            .name()
            .unwrap()
            .parse::<usize>()
            .unwrap();
        let original = ids.contains(&object.id());
        let (domain, points) = if let Some(curve) = object.geometry().curve_ref() {
            let domain = curve.domain();
            let (a, b) = (*domain.start(), *domain.end());
            (
                json!([a, b]),
                (0..=32)
                    .map(|i| {
                        curve
                            .evaluate(a + (b - a) * f64::from(i) / 32.)
                            .map(|p| p.to_array())
                    })
                    .collect::<Result<Vec<_>, _>>()?,
            )
        } else if let Geometry::NurbsSurface(s) = object.geometry() {
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
            (
                json!([[*u.start(), *u.end()], [*v.start(), *v.end()]]),
                points,
            )
        } else {
            return Err(error());
        };
        let key = points
            .iter()
            .flatten()
            .map(|v| (v * 1e8).round() / 1e8)
            .collect::<Vec<_>>();
        records.push((
            (source, !original, key),
            json!({
                "source": source,
                "original": original,
                "selected": document.is_selected(object.id()),
                "domain": domain,
                "points": points,
            }),
        ));
    }
    records.sort_by(|(a, _), (b, _)| a.partial_cmp(b).expect("finite sort coordinates"));
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
        json!({
            "objects": records.into_iter().map(|(_, v)| v).collect::<Vec<_>>(),
            "groups": groups,
        }),
        0,
    ))
}
