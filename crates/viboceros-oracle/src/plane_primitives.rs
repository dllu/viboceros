//! Actual primitive commands under an explicit construction plane.
use super::*;
use viboceros_command::CommandContext;

#[cfg(test)]
mod tests {
    #[test]
    fn plane_primitive_fixture_checks_every_command_on_oriented_frames() {
        let request: crate::ProbeRequest = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/plane_primitives.json"
        ))
        .unwrap();
        let response = crate::run_request(&request).unwrap();
        assert_eq!(response.results.len(), 57);
        assert!(response.results.iter().all(|r| r.elapsed_ns == 0));
        let record = |id| &response.results.iter().find(|r| r.id == id).unwrap().value;
        assert_eq!(
            record("rectangle-nn")["domain"],
            serde_json::json!([0.0, 20.0])
        );
        assert_eq!(
            record("Circle-normal-pick")["points"][0],
            serde_json::json!([0.0, 0.0, 5.0])
        );
        for result in response.results.iter().filter(|r| r.id.starts_with("Box-")) {
            assert_eq!(result.value["solid"], true);
            assert_eq!(result.value["vertices"].as_array().unwrap().len(), 8);
            assert_eq!(result.value["edges"].as_array().unwrap().len(), 12);
            let faces = result.value["faces"].as_array().unwrap();
            assert_eq!(faces.len(), 6);
            assert!(
                faces
                    .iter()
                    .all(|f| f["samples"].as_array().unwrap().len() == 81
                        && f["loops"].as_array().unwrap().len() == 1)
            );
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct PlanePrimitiveFixture {
    pub origin: [f64; 3],
    pub x_axis: [f64; 3],
    pub y_axis: [f64; 3],
    pub primitive: String,
    pub points: Vec<[f64; 3]>,
    pub value: Option<f64>,
    #[serde(default)]
    pub raw_representation: bool,
}

pub(super) fn run(
    f: &PlanePrimitiveFixture,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    let plane = Frame3::try_from_directions(
        Point3::try_from(f.origin)?,
        Vector3::try_from(f.x_axis)?,
        Vector3::try_from(f.y_axis)?,
        tolerance,
    )?;
    let (prefix, expected) = match f.primitive.as_str() {
        "Circle" => ("Circle", if f.value.is_some() { 1 } else { 2 }),
        "Polygon" => ("Polygon 5", if f.value.is_some() { 1 } else { 2 }),
        "Rectangle" | "MeshPlane" => (f.primitive.as_str(), 2),
        "Box" | "MeshBox" => (f.primitive.as_str(), if f.value.is_some() { 2 } else { 3 }),
        _ => return Err(ProbeError::FixtureInvariant("unsupported plane primitive")),
    };
    if f.points.len() != expected
        || (matches!(f.primitive.as_str(), "Rectangle" | "MeshPlane") && f.value.is_some())
    {
        return Err(ProbeError::FixtureInvariant(
            "incorrect primitive arguments",
        ));
    }
    let mut command = prefix.to_owned();
    for p in &f.points {
        Point3::try_from(*p)?;
        command.push_str(&format!(" {},{},{}", p[0], p[1], p[2]));
    }
    if let Some(value) = f.value {
        if !value.is_finite() {
            return Err(ProbeError::FixtureInvariant("nonfinite primitive size"));
        }
        command.push_str(&format!(" {value}"));
    }
    if f.primitive == "MeshPlane" {
        command.push_str(" XCount=2 YCount=3");
    }
    if f.primitive == "MeshBox" {
        command.push_str(" XCount=2 YCount=3 ZCount=2");
    }
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::new(tolerance);
    registry.execute_in_context(
        &mut document,
        &command,
        CommandContext {
            construction_plane: plane,
        },
    )?;
    if document.objects().len() != 1 {
        return Err(ProbeError::FixtureInvariant("expected one primitive"));
    }
    let geometry = document.objects().next().unwrap().geometry();
    let value = match geometry {
        Geometry::Brep(brep) => {
            let record = super::brep_interchange::geometry_record(brep)?;
            if f.raw_representation {
                record
            } else {
                canonical_box_record(&record)?
            }
        }
        Geometry::Mesh(mesh) => {
            let record = polygon_mesh_value(mesh);
            if f.primitive == "MeshBox" && !f.raw_representation {
                canonical_mesh_record(&record)
            } else {
                record
            }
        }
        _ => curve_native::command_record(
            geometry
                .curve_ref()
                .ok_or(ProbeError::FixtureInvariant("expected a curve"))?,
        )?,
    };
    Ok((value, 0))
}

// Box commands may return an extrusion-derived B-rep in Rhino. Compare the
// complete boundary independently of its vertex/edge IDs and face UV axes.
// Quantization is used ONLY for ordering; reported coordinates stay unrounded.
fn point_value(value: &Value) -> [f64; 3] {
    std::array::from_fn(|i| value[i].as_f64().expect("recorded coordinate"))
}

fn canonical_mesh_record(record: &Value) -> Value {
    let raw_vertices = record["vertices"].as_array().unwrap();
    let vertices = sorted_points(&record["vertices"]);
    let mut faces = record["faces"]
        .as_array()
        .unwrap()
        .iter()
        .map(|face| {
            let mut points = face
                .as_array()
                .unwrap()
                .iter()
                .map(|i| raw_vertices[i.as_u64().unwrap() as usize].clone())
                .collect::<Vec<_>>();
            let start = (0..points.len())
                .min_by_key(|&i| point_key(&points[i]))
                .unwrap();
            points.rotate_left(start);
            points
        })
        .collect::<Vec<_>>();
    faces.sort_by_key(|points| points.iter().map(point_key).collect::<Vec<_>>());
    json!({"vertices":vertices,"faces":faces})
}
fn point_key(value: &Value) -> [i64; 3] {
    point_value(value).map(|v| (v * 1e6).round() as i64)
}
fn sorted_points(value: &Value) -> Vec<Value> {
    let mut points = value.as_array().expect("recorded samples").clone();
    points.sort_by_key(point_key);
    points
}
fn canonical_box_record(record: &Value) -> Result<Value, ProbeError> {
    let mut vertices = record["vertices"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v["point"].clone())
        .collect::<Vec<_>>();
    vertices.sort_by_key(point_key);
    let mut edges = record["edges"]
        .as_array()
        .unwrap()
        .iter()
        .map(|edge| {
            let mut points = edge["curve"]["samples"].as_array().unwrap().clone();
            if point_key(&points[0]) > point_key(points.last().unwrap()) {
                points.reverse();
            }
            points
        })
        .collect::<Vec<_>>();
    edges.sort_by_key(|e| [point_key(&e[0]), point_key(e.last().unwrap())]);
    let mut faces = Vec::new();
    for (i, face) in record["faces"].as_array().unwrap().iter().enumerate() {
        let samples = face["samples"].as_array().unwrap();
        let p = Point3::try_from(point_value(&samples[0]))?;
        let mut normal = p
            .vector_to(Point3::try_from(point_value(&samples[8]))?)?
            .cross(p.vector_to(Point3::try_from(point_value(&samples[72]))?)?)?
            .normalized_nonzero()?
            .as_vector();
        let reversed = record["topology"]["faces"][i]["reversed"]
            .as_bool()
            .unwrap();
        if reversed {
            normal = normal.scaled(-1.0)?;
        }
        let mut loops = Vec::new();
        for boundary in face["loops"].as_array().unwrap() {
            let mut trims = boundary
                .as_array()
                .unwrap()
                .iter()
                .map(|trim| trim["lifted"].as_array().unwrap().clone())
                .collect::<Vec<_>>();
            if reversed {
                trims.reverse();
                for trim in &mut trims {
                    trim.reverse();
                }
            }
            let start = (0..trims.len())
                .min_by_key(|&j| point_key(&trims[j][0]))
                .unwrap();
            trims.rotate_left(start);
            loops.push(trims);
        }
        faces.push(json!({"samples":sorted_points(&face["samples"]), "normal":normal.to_array(), "loops":loops}));
    }
    faces.sort_by_key(|face| {
        let points = face["samples"].as_array().unwrap();
        let center: [f64; 3] = std::array::from_fn(|axis| {
            points
                .iter()
                .map(|p| p[axis].as_f64().unwrap())
                .sum::<f64>()
                / points.len() as f64
        });
        center.map(|v| (v * 1e6).round() as i64)
    });
    Ok(json!({"solid":record["topology"]["solid"],"vertices":vertices,"edges":edges,"faces":faces}))
}
