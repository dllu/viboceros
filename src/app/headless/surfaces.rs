//! Exact tensor-product surface admission, patch extraction, and native UV evaluation.
use super::*;
use std::collections::{BTreeMap, BTreeSet, btree_map::Entry};
use viboceros_command::{ComponentSelectionKind, component_selection::ComponentPick};
use viboceros_document::{ColorRgb, Geometry, ObjectAttributes};
use viboceros_geometry::{
    Brep, GeometryError, NurbsCurve2, NurbsSurface, ParameterSide, Point2, WeightedPoint2,
    WeightedPoint3,
};

#[derive(Deserialize)]
pub(super) struct Definition {
    degree_u: usize,
    degree_v: usize,
    knots_u: Vec<f64>,
    knots_v: Vec<f64>,
    control_net: Vec<Vec<[f64; 3]>>,
    weights: Option<Vec<Vec<f64>>>,
}

#[derive(Deserialize)]
pub(super) struct Patch {
    domain: Option<[[f64; 2]; 2]>,
    name: Option<String>,
    color: Option<[u8; 3]>,
    opacity: Option<f64>,
    #[serde(default)]
    trim_loops: Vec<Vec<TrimCurve>>,
}

#[derive(Deserialize)]
pub(super) struct ChartEdit {
    face: usize,
    surface: Definition,
    trims: Vec<(usize, usize, TrimCurve)>,
}

#[derive(Deserialize)]
struct TrimCurve {
    degree: usize,
    knots: Vec<f64>,
    control_points: Vec<[f64; 2]>,
    weights: Option<Vec<f64>>,
}

impl TrimCurve {
    fn build(self) -> Result<NurbsCurve2> {
        if self
            .weights
            .as_ref()
            .is_some_and(|weights| weights.len() != self.control_points.len())
        {
            return Err("trim weights must match the trim controls".into());
        }
        let mut controls = Vec::with_capacity(self.control_points.len());
        for (index, uv) in self.control_points.into_iter().enumerate() {
            let weight = self.weights.as_ref().map_or(1., |weights| weights[index]);
            controls.push(WeightedPoint2::try_new(Point2::try_from(uv)?, weight)?);
        }
        Ok(NurbsCurve2::try_new_rational(
            self.degree,
            controls,
            self.knots,
        )?)
    }
}

#[derive(Deserialize)]
pub(super) struct Sample {
    uv: [f64; 2],
    #[serde(default)]
    side_u: Side,
    #[serde(default)]
    side_v: Side,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "snake_case")]
enum Side {
    Left,
    #[default]
    Right,
}

impl From<Side> for ParameterSide {
    fn from(value: Side) -> Self {
        match value {
            Side::Left => Self::Left,
            Side::Right => Self::Right,
        }
    }
}

impl Definition {
    fn build(self) -> Result<NurbsSurface> {
        let nu = self.control_net.len();
        let nv = self.control_net.first().map_or(0, Vec::len);
        if nu == 0 || nv == 0 || self.control_net.iter().any(|row| row.len() != nv) {
            return Err("surface control net must be a nonempty rectangular [u][v] grid".into());
        }
        if self
            .weights
            .as_ref()
            .is_some_and(|weights| weights.len() != nu || weights.iter().any(|row| row.len() != nv))
        {
            return Err("surface weights must match the control net".into());
        }
        let mut controls = Vec::with_capacity(nu * nv);
        for v in 0..nv {
            for (u, row) in self.control_net.iter().enumerate() {
                let point = Point3::try_from(row[v])?;
                let weight = self.weights.as_ref().map_or(1.0, |weights| weights[u][v]);
                controls.push(WeightedPoint3::try_new(point, weight)?);
            }
        }
        Ok(NurbsSurface::try_new_rational(
            self.degree_u,
            self.degree_v,
            nu,
            nv,
            controls,
            self.knots_u,
            self.knots_v,
        )?)
    }
}

pub(super) fn add_patches(
    document: &mut Document,
    definition: Definition,
    patches: Vec<Patch>,
) -> Result<Value> {
    if patches.is_empty() {
        return Err("supply at least one surface patch".into());
    }
    let parent = definition.build()?;
    let mut staged = Vec::with_capacity(patches.len());
    for patch in patches {
        let surface = if let Some([[u0, u1], [v0, v1]]) = patch.domain {
            parent.try_trimmed(u0..=u1, v0..=v1)?
        } else {
            parent.clone()
        };
        let mut attributes = ObjectAttributes::on_layer(document.current_layer_id());
        if let Some(name) = patch.name {
            attributes = attributes.with_name(name);
        }
        if let Some([r, g, b]) = patch.color {
            attributes = attributes.with_object_color(ColorRgb::new(r, g, b));
        }
        if let Some(opacity) = patch.opacity {
            attributes = attributes.try_with_opacity(opacity)?;
        }
        let geometry = if patch.trim_loops.is_empty() {
            Geometry::NurbsSurface(surface)
        } else {
            let loops = patch
                .trim_loops
                .into_iter()
                .map(|boundary| {
                    boundary
                        .into_iter()
                        .map(TrimCurve::build)
                        .collect::<Result<Vec<_>>>()
                })
                .collect::<Result<Vec<_>>>()?;
            Geometry::Brep(Brep::try_parameter_face(
                surface,
                loops,
                document.tolerance(),
            )?)
        };
        staged.push((geometry, attributes));
    }
    let ids = edit_document_transaction(document, "Add NURBS surface patches", |document| {
        staged
            .into_iter()
            .map(|(geometry, attributes)| {
                document.add_geometry_with_attributes(geometry, attributes)
            })
            .collect::<std::result::Result<Vec<_>, _>>()
    })?;
    Ok(json!(
        ids.iter().map(ToString::to_string).collect::<Vec<_>>()
    ))
}

pub(super) fn sew_edges(
    document: &mut Document,
    pairs: Vec<(ComponentRef, ComponentRef, bool)>,
    anchor: Option<&str>,
    vertex_distance: Option<f64>,
) -> Result<Value> {
    if pairs.is_empty() {
        return Err("supply at least one B-rep edge pair".into());
    }
    let tolerance = document.tolerance();
    let mut offsets = BTreeMap::new();
    let mut sources = Vec::new();
    let mut parts = Vec::new();
    let mut edge_count = 0usize;
    let pick_edge = |reference: ComponentRef| -> Result<ComponentPick> {
        let pick = reference.pick(|id| model::revision(document, id))?;
        if pick.kind != ComponentSelectionKind::BrepEdge {
            return Err("sewing requires B-rep edge references".into());
        }
        Ok(pick)
    };
    let picks = pairs
        .into_iter()
        .map(|(a, b, reversed)| Ok((pick_edge(a)?, pick_edge(b)?, reversed)))
        .collect::<Result<Vec<_>>>()?;
    let mut combined_edge = |pick: ComponentPick| -> Result<usize> {
        let (offset, count) = match offsets.entry(pick.object) {
            Entry::Occupied(entry) => *entry.get(),
            Entry::Vacant(entry) => {
                let object = document
                    .object(pick.object)
                    .ok_or_else(|| format!("unknown object {}", pick.object))?;
                let part = match object.geometry() {
                    Geometry::Brep(brep) => brep.clone(),
                    // Use the same edge numbering as the SDK component query.
                    Geometry::NurbsSurface(surface) => {
                        Brep::try_surface_face(surface.clone(), tolerance)?
                    }
                    _ => return Err("sewing requires B-rep or NURBS surface objects".into()),
                };
                let count = part.edges().len();
                let offset = edge_count;
                edge_count += count;
                sources.push(pick.object);
                parts.push(part);
                *entry.insert((offset, count))
            }
        };
        if pick.index >= count {
            return Err("B-rep edge index is out of range".into());
        }
        Ok(offset + pick.index)
    };
    if let Some(anchor) = anchor {
        let id = model::object_id(anchor)?;
        let pick = picks
            .iter()
            .flat_map(|(a, b, _)| [a, b])
            .find(|pick| pick.object == id)
            .ok_or("the orientation anchor must supply at least one paired edge")?;
        combined_edge(*pick)?;
    }
    let edge_pairs = picks
        .iter()
        .map(|&(a, b, reversed)| Ok((combined_edge(a)?, combined_edge(b)?, reversed)))
        .collect::<Result<Vec<_>>>()?;
    let attributes = document.object(sources[0]).unwrap().attributes().clone();
    let combined = Brep::try_combine(parts, tolerance)?;
    let sewn = combined
        .try_join_edge_pairs_with_vertex_distance(
            &edge_pairs,
            tolerance.absolute(),
            vertex_distance.unwrap_or(tolerance.absolute()),
            tolerance,
        )
        .map_err(|error| -> SessionError {
            if let GeometryError::UncertifiedBrepEdgePair { pair, .. } = error {
                let (a, b, _) = picks[pair];
                format!(
                    "{error}; input {} edge {}, input {} edge {}",
                    a.object, a.index, b.object, b.index
                )
                .into()
            } else {
                error.into()
            }
        })?;
    let id = edit_document_transaction(document, "Sew B-rep edges", |document| {
        document.delete_objects(sources)?;
        document.add_geometry_with_attributes(Geometry::Brep(sewn), attributes)
    })?;
    Ok(json!(id.to_string()))
}

pub(super) fn edit_face_charts(
    document: &mut Document,
    id: &str,
    edits: Vec<ChartEdit>,
) -> Result<Value> {
    let id = model::object_id(id)?;
    let Some(Geometry::Brep(source)) = document.object(id).map(|object| object.geometry()) else {
        return Err("face chart editing requires a B-rep object".into());
    };
    let mut edited = source.clone();
    let mut seen = BTreeSet::new();
    for edit in edits {
        if !seen.insert(edit.face) {
            return Err("face chart edit indices must be unique".into());
        }
        let trims = edit
            .trims
            .into_iter()
            .map(|(boundary, index, curve)| Ok((boundary, index, curve.build()?)))
            .collect::<Result<Vec<_>>>()?;
        edited
            .try_edit_face_chart(
                edit.face,
                edit.surface.build()?,
                trims,
                document.tolerance(),
            )
            .map_err(|error| format!("face {}: {error}", edit.face))?;
    }
    document.replace_object_geometries([(id, Geometry::Brep(edited))])?;
    Ok(Value::Null)
}

fn surface<'a>(document: &'a Document, id: &str, face: Option<usize>) -> Result<&'a NurbsSurface> {
    let id = model::object_id(id)?;
    let object = document
        .object(id)
        .ok_or_else(|| format!("unknown object {id}"))?;
    match object.geometry() {
        Geometry::NurbsSurface(surface) if face.is_none_or(|index| index == 0) => Ok(surface),
        Geometry::Brep(brep) if face.is_some() || brep.faces().len() == 1 => brep
            .faces()
            .get(face.unwrap_or(0))
            .map(|face| face.surface())
            .ok_or_else(|| "B-rep face index is out of range".into()),
        Geometry::Brep(_) => Err("specify a zero-based face index for a multi-face B-rep".into()),
        _ => Err("object does not identify a NURBS surface".into()),
    }
}

pub(super) fn definition(document: &Document, id: &str, face: Option<usize>) -> Result<Value> {
    let surface = surface(document, id, face)?;
    let mut points = Vec::new();
    let mut weights = Vec::new();
    for u in 0..surface.control_point_count_u() {
        let column =
            (0..surface.control_point_count_v()).map(|v| surface.control_point(u, v).unwrap());
        let (column_points, column_weights): (Vec<_>, Vec<_>) = column
            .map(|control| (control.point().to_array(), control.weight()))
            .unzip();
        points.push(column_points);
        weights.push(column_weights);
    }
    Ok(json!({
        "degree_u": surface.degree_u(), "degree_v": surface.degree_v(),
        "knots_u": surface.knots_u(), "knots_v": surface.knots_v(),
        "control_net": points, "weights": weights,
    }))
}

pub(super) fn jets(
    document: &Document,
    id: &str,
    face: Option<usize>,
    samples: Vec<Sample>,
) -> Result<Value> {
    let surface = surface(document, id, face)?;
    let mut results = Vec::with_capacity(samples.len());
    for sample in samples {
        let [u, v] = sample.uv;
        let jet = surface.evaluate_with_second_derivatives_on_sides(
            u,
            v,
            sample.side_u.into(),
            sample.side_v.into(),
        )?;
        results.push(json!({
            "point": jet.point.to_array(), "du": jet.derivative_u.to_array(), "dv": jet.derivative_v.to_array(),
            "duu": jet.derivative_uu.to_array(), "duv": jet.derivative_uv.to_array(), "dvv": jet.derivative_vv.to_array(),
        }));
    }
    Ok(json!(results))
}

pub(super) fn pullback_edge(
    document: &Document,
    reference: ComponentRef,
    id: &str,
    face: Option<usize>,
    reversed: bool,
    endpoints: Option<[[f64; 2]; 2]>,
) -> Result<Value> {
    let pick = reference.pick(|id| model::revision(document, id))?;
    if pick.kind != ComponentSelectionKind::BrepEdge {
        return Err("pullback requires a B-rep edge reference".into());
    }
    let Some(Geometry::Brep(brep)) = document.object(pick.object).map(|o| o.geometry()) else {
        return Err("pullback requires a B-rep source object".into());
    };
    let spatial = brep
        .edges()
        .get(pick.index)
        .ok_or("B-rep edge index is out of range")?
        .curve();
    let surface = surface(document, id, face)?;
    let tolerance = document.tolerance();
    let curve = if let Some([a, b]) = endpoints {
        let mut endpoints = [Point2::try_from(a)?, Point2::try_from(b)?];
        if reversed {
            endpoints.reverse();
        }
        surface.try_pullback_curve_certified_with_endpoints(spatial, endpoints, tolerance)?
    } else {
        surface.try_pullback_curve_certified(spatial, tolerance)?
    };
    let curve = if reversed { curve.reversed()? } else { curve };
    let (controls, weights): (Vec<_>, Vec<_>) = curve
        .control_points()
        .iter()
        .map(|p| (p.point().to_array(), p.weight()))
        .unzip();
    Ok(json!({
        "degree": curve.degree(), "knots": curve.knots(),
        "control_points": controls, "weights": weights,
    }))
}
