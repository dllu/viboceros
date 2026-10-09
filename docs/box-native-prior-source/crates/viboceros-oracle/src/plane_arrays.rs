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
    SurfaceOrBrep(SurfaceOrBrepSource),
    Curve(CurveInput),
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SurfaceOrBrepSource {
    Surface {
        #[serde(flatten)]
        surface: NurbsSurfaceDefinition,
    },
    Brep {
        #[serde(flatten)]
        fixture: Box<TrimmedBrepFixture>,
    },
    BoxBrep {
        min: [f64; 3],
        max: [f64; 3],
    },
}

impl ArraySource {
    pub(super) fn geometry(&self, tolerance: Tolerance) -> Result<Geometry, ProbeError> {
        Ok(match self {
            Self::SurfaceOrBrep(SurfaceOrBrepSource::Surface { surface }) => {
                Geometry::NurbsSurface(nurbs_surface_from_definition(surface)?)
            }
            Self::SurfaceOrBrep(SurfaceOrBrepSource::Brep { fixture }) => {
                Geometry::Brep(trimmed_brep::build(fixture, tolerance)?)
            }
            Self::SurfaceOrBrep(SurfaceOrBrepSource::BoxBrep { min, max }) => {
                let frame = viboceros_command::CommandContext::default().construction_plane;
                Geometry::Brep(box_source(
                    frame,
                    std::array::from_fn(|i| [min[i], max[i]]),
                    tolerance,
                )?)
            }
            Self::Curve(curve) => curve.geometry()?.into(),
        })
    }
}

// The fixture mirrors the public SDK's box face order and physical UV
// intervals; a geometry-only constructor's normalized UVs are a different input.
fn box_source(
    frame: Frame3,
    bounds: [[f64; 2]; 3],
    tolerance: Tolerance,
) -> Result<Brep, GeometryError> {
    use viboceros_geometry::{BrepFace, BrepLoop, BrepTrim, NurbsCurve2, Point2, WeightedPoint2};
    let source = Brep::try_box(frame, bounds, tolerance)?;
    let mut faces = Vec::new();
    for index in [2, 5, 3, 4, 0, 1] {
        let face = &source.faces()[index];
        let surface = face.surface();
        let origin = surface.control_point(0, 0).unwrap().point();
        let width = origin.distance_to(surface.control_point(1, 0).unwrap().point())?;
        let height = origin.distance_to(surface.control_point(0, 1).unwrap().point())?;
        let mut loops = Vec::new();
        for boundary in face.loops() {
            let mut trims = Vec::new();
            for trim in boundary.trims() {
                let curve = trim.curve();
                let controls = curve
                    .control_points()
                    .iter()
                    .map(|p| {
                        WeightedPoint2::try_new(
                            Point2::try_new(p.point().x() * width, p.point().y() * height)?,
                            p.weight(),
                        )
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                trims.push(BrepTrim::try_new(
                    trim.vertices(),
                    trim.edge(),
                    trim.is_reversed_3d(),
                    NurbsCurve2::try_new_rational(
                        curve.degree(),
                        controls,
                        curve.knots().to_vec(),
                    )?,
                    trim.trim_type(),
                    trim.iso(),
                    trim.tolerance(),
                )?);
            }
            loops.push(BrepLoop::try_new(boundary.loop_type(), trims)?);
        }
        faces.push(BrepFace::try_new(
            surface.try_reparameterized(0. ..=width, 0. ..=height)?,
            face.is_reversed(),
            loops,
        )?);
    }
    Brep::try_new(
        source.vertices().to_vec(),
        source.edges().to_vec(),
        faces,
        tolerance,
    )
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
        ids.push(document.add_geometry_with_attributes(source.geometry(tolerance)?, attributes)?);
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
        } else if let Geometry::Brep(brep) = object.geometry() {
            brep_record(brep)?
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
        // This older layout probe compares populated groups, as does its
        // Rhino worker. group_memberships separately checks the complete table.
        .filter(|group| group.members().len() > 0)
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

pub(super) fn brep_record(brep: &Brep) -> Result<(Value, Vec<[f64; 3]>), GeometryError> {
    let mut domains = Vec::new();
    let mut points = Vec::new();
    for face in brep.faces() {
        let s = face.surface();
        let u = s.domain_u();
        let v = s.domain_v();
        domains.push([[*u.start(), *u.end()], [*v.start(), *v.end()]]);
        for j in 0..=4 {
            for i in 0..=4 {
                points.push(
                    s.evaluate(
                        u.start() + (u.end() - u.start()) * f64::from(i) / 4.,
                        v.start() + (v.end() - v.start()) * f64::from(j) / 4.,
                    )?
                    .to_array(),
                );
            }
        }
        for trim in face.loops().iter().flat_map(|l| l.trims()) {
            let d = trim.curve().domain();
            for i in 0..=8 {
                let uv = trim
                    .curve()
                    .evaluate(d.start() + (d.end() - d.start()) * f64::from(i) / 8.)?;
                points.push(s.evaluate(uv.x(), uv.y())?.to_array());
            }
        }
    }
    Ok((json!(domains), points))
}
