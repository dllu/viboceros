//! Canonical surface definitions, sparse edge incidence and boundary samples.
use super::*;
use std::borrow::Cow;

fn canonical(geometry: &Geometry, tolerance: Tolerance) -> Result<Cow<'_, Brep>, ProbeError> {
    match geometry {
        Geometry::Brep(b) => Ok(Cow::Borrowed(b)),
        Geometry::NurbsSurface(s) => Ok(Cow::Owned(Brep::try_surface_face(s.clone(), tolerance)?)),
        _ => Err(ProbeError::FixtureInvariant("revolution surface required")),
    }
}
fn surface_definition(surface: &NurbsSurface) -> Value {
    let mut value = nurbs_surface_definition_value(surface);
    for name in ["knots_u", "knots_v"] {
        let k = value[name].as_array_mut().unwrap();
        let last = k.len() - 1;
        k[0] = k[1].clone();
        k[last] = k[last - 1].clone();
    }
    value
}

pub fn geometry_record(geometry: &Geometry, tolerance: Tolerance) -> Result<Value, ProbeError> {
    Ok(record(canonical(geometry, tolerance)?.as_ref()))
}

fn record(b: &Brep) -> Value {
    let mut types = b
        .faces()
        .iter()
        .flat_map(|f| f.loops())
        .flat_map(|l| l.trims())
        .map(|t| format!("{:?}", t.trim_type()))
        .collect::<Vec<_>>();
    types.sort();
    let mut valences = (0..b.edges().len())
        .map(|i| match b.edge_use_count(i).unwrap() {
            0 => 0,
            1 => 1,
            2 => 2,
            _ => 3,
        })
        .collect::<Vec<_>>();
    valences.sort();
    json!({"vertices":b.vertices().len(),"edges":b.edges().len(),"faces":b.faces().len(),"solid":b.is_solid(),"valences":valences,"trim_types":types,
        "surfaces":b.faces().iter().map(|f|json!({"surface":surface_definition(f.surface()),"reversed":f.is_reversed(),"loops":f.loops().len()})).collect::<Vec<_>>()})
}
pub fn snapshot(doc: &Document, source: ObjectId) -> Result<Value, ProbeError> {
    let mut rows = Vec::new();
    for obj in doc.objects() {
        let mut row =
            json!({"source":obj.id()==source,"selected":if doc.is_selected(obj.id()){2}else{0}});
        if let Geometry::NurbsCurve(c) = obj.geometry() {
            row["kind"] = json!("curve");
            row["curve"] = nurbs_curve_definition_value(c);
            let k = row["curve"]["knots"].as_array_mut().unwrap();
            let last = k.len() - 1;
            k[0] = k[1].clone();
            k[last] = k[last - 1].clone();
        } else {
            row["kind"] = json!("surface");
            row["brep"] = record(canonical(obj.geometry(), doc.tolerance())?.as_ref());
        }
        rows.push(row);
    }
    Ok(json!(rows))
}
pub fn adjacency(doc: &Document, source: ObjectId) -> Result<Value, ProbeError> {
    let mut outputs = Vec::new();
    for obj in doc.objects().filter(|o| o.id() != source) {
        let b = canonical(obj.geometry(), doc.tolerance())?;
        let mut uses = vec![Vec::new(); b.edges().len()];
        for (i, face) in b.faces().iter().enumerate() {
            for trim in face.loops().iter().flat_map(|l| l.trims()) {
                if let Some(edge) = trim.edge() {
                    uses[edge].push(i);
                }
            }
        }
        let mut pairs = uses
            .into_iter()
            .filter(|u| u.len() == 2)
            .collect::<Vec<_>>();
        pairs.sort();
        outputs.push(pairs);
    }
    Ok(json!(outputs))
}
pub fn boundary_samples(doc: &Document, source: ObjectId) -> Result<Value, ProbeError> {
    let mut outputs = Vec::new();
    for obj in doc.objects().filter(|o| o.id() != source) {
        let b = canonical(obj.geometry(), doc.tolerance())?;
        let mut uses = vec![Vec::new(); b.edges().len()];
        for (i, f) in b.faces().iter().enumerate() {
            for t in f.loops().iter().flat_map(|l| l.trims()) {
                if let Some(edge) = t.edge() {
                    uses[edge].push(i);
                }
            }
        }
        let mut rows = Vec::new();
        for (i, edge) in b.edges().iter().enumerate() {
            let ends = edge.vertices().map(|i| b.vertices()[i].point().to_array());
            let points = (0..=32)
                .map(|j| {
                    edge.curve()
                        .evaluate(edge.curve().parameter_at(j as f64 / 32.)?)
                        .map(|p| p.to_array())
                })
                .collect::<Result<Vec<_>, _>>()?;
            rows.push(json!({"faces":uses[i],"ends":ends,"points":points,"curve":nurbs_curve_definition_value(edge.curve())}));
        }
        outputs.push(rows);
    }
    Ok(json!(outputs))
}
