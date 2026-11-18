//! Exact tensor-product surface admission, patch extraction, and native UV evaluation.
use super::*;
use viboceros_document::{ColorRgb, Geometry, ObjectAttributes};
use viboceros_geometry::{
    Brep, NurbsCurve2, NurbsSurface, ParameterSide, Point2, WeightedPoint2, WeightedPoint3,
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
