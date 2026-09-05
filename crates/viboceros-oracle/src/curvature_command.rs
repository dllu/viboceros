use super::*;

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct CurvatureCommandFixture {
    #[serde(default)]
    pub curve: Option<NurbsCurveDefinition>,
    #[serde(default)]
    pub surface: Option<NurbsSurfaceDefinition>,
    pub point: [f64; 3],
    #[serde(default)]
    pub mark: bool,
    #[serde(default)]
    pub as_brep: bool,
    #[serde(default)]
    pub reverse_face: bool,
}

pub(super) fn run(
    fixture: &CurvatureCommandFixture,
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    if fixture.curve.is_some() && (fixture.as_brep || fixture.reverse_face) {
        return Err(ProbeError::FixtureInvariant(
            "a curve cannot be a curvature B-rep source",
        ));
    }
    let source = match (&fixture.curve, &fixture.surface) {
        (Some(c), None) => Geometry::NurbsCurve(nurbs_curve_from_definition(c)?),
        (None, Some(s)) => {
            let surface = nurbs_surface_from_definition(s)?;
            if fixture.as_brep || fixture.reverse_face {
                let brep = Brep::try_surface_face(surface, tolerance)?;
                Geometry::Brep(if fixture.reverse_face {
                    brep.reversed()
                } else {
                    brep
                })
            } else {
                Geometry::NurbsSurface(surface)
            }
        }
        _ => {
            return Err(ProbeError::FixtureInvariant(
                "curvature command needs exactly one curve or surface",
            ));
        }
    };
    let registry = CommandRegistry::with_builtins();
    let command = format!(
        "Curvature MarkCurvature={} {},{},{}",
        if fixture.mark { "Yes" } else { "No" },
        fixture.point[0],
        fixture.point[1],
        fixture.point[2]
    );
    let compute = || -> Result<Value, ProbeError> {
        let mut document = Document::new(tolerance);
        let id = document.add_geometry(source.clone())?;
        document.select_objects_direct([id], SelectionMode::Replace)?;
        registry.execute(&mut document, "SetObjectName curvature-source")?;
        let report = registry.execute(&mut document, &command)?;
        let reported = report.starts_with("Curve curvature at parameter")
            || report.starts_with("Surface curvature");
        if !reported {
            return Err(ProbeError::FixtureInvariant(
                "curvature command produced no measurement",
            ));
        }
        let mut outputs = document
            .objects()
            .filter(|o| o.id() != id)
            .map(|o| {
                let mut value = marker(o.geometry())?;
                value["name"] = json!(o.attributes().name());
                value["selected"] = json!(document.is_selected(o.id()));
                value["group_count"] = json!(
                    document
                        .groups()
                        .filter(|g| g.members().any(|id| id == o.id()))
                        .count()
                );
                Ok::<_, ProbeError>(value)
            })
            .collect::<Result<Vec<_>, _>>()?;
        outputs.sort_by(|a, b| {
            a["kind"]
                .as_str()
                .cmp(&b["kind"].as_str())
                .then_with(|| {
                    center(a)
                        .iter()
                        .zip(center(b))
                        .map(|(x, y)| x.partial_cmp(&y).unwrap())
                        .find(|o| o.is_ne())
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .then_with(|| {
                    let tensor = |v: &Value| {
                        v["half_chord_tensor"]
                            .as_array()
                            .into_iter()
                            .flatten()
                            .flat_map(|row| row.as_array().into_iter().flatten())
                            .map(|n| n.as_f64().unwrap())
                            .collect::<Vec<_>>()
                    };
                    tensor(a)
                        .into_iter()
                        .zip(tensor(b))
                        .map(|(x, y)| x.partial_cmp(&y).unwrap())
                        .find(|o| o.is_ne())
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .then_with(|| {
                    b["radius"]
                        .as_f64()
                        .unwrap_or(0.0)
                        .total_cmp(&a["radius"].as_f64().unwrap_or(0.0))
                })
        });
        let original = document.object(id);
        Ok(
            json!({"outputs":outputs,"reported":reported,"source_present":original.is_some(),"source_selected":document.is_selected(id),
            "source_name":original.and_then(|o|o.attributes().name()),
            "source_geometry_unchanged":original.is_some_and(|o|o.geometry()==&source)}),
        )
    };
    let mut value = compute()?;
    let start = Instant::now();
    for _ in 0..iterations {
        value = black_box(compute()?);
    }
    Ok((
        value,
        u64::try_from(start.elapsed().as_nanos()).map_err(|_| ProbeError::TimingOverflow)?,
    ))
}

fn center(value: &Value) -> Vec<f64> {
    value
        .get("center")
        .or_else(|| value.get("point"))
        .and_then(Value::as_array)
        .map(|v| v.iter().map(|a| a.as_f64().unwrap()).collect())
        .unwrap_or_default()
}
fn outer(v: [f64; 3]) -> Result<[[f64; 3]; 3], ProbeError> {
    let tensor: [[f64; 3]; 3] = std::array::from_fn(|i| std::array::from_fn(|j| v[i] * v[j]));
    if tensor.iter().flatten().any(|a| !a.is_finite()) {
        return Err(ProbeError::FixtureInvariant(
            "curvature marker tensor is not representable",
        ));
    }
    Ok(tensor)
}
fn marker(geometry: &Geometry) -> Result<Value, ProbeError> {
    Ok(match geometry {
        Geometry::Line(line) => {
            let start = line.start().to_array();
            let end = line.end().to_array();
            let center: [f64; 3] = std::array::from_fn(|i| start[i] * 0.5 + end[i] * 0.5);
            let chord = std::array::from_fn(|i| start[i] * 0.5 - end[i] * 0.5);
            json!({"kind":"line","center":center,"half_chord_tensor":outer(chord)?})
        }
        Geometry::Point(p) => json!({"kind":"point","point":p.to_array()}),
        Geometry::Circle(c) => {
            json!({"kind":"circle","center":c.center().to_array(),"radius":c.radius(),"plane_tensor":outer(c.normal()?.as_vector().to_array())?})
        }
        Geometry::Arc(a) => {
            let start = a.start()?.to_array();
            let end = a.end()?.to_array();
            let midpoint: [f64; 3] = std::array::from_fn(|i| start[i] * 0.5 + end[i] * 0.5);
            let chord = std::array::from_fn(|i| start[i] * 0.5 - end[i] * 0.5);
            json!({"kind":"arc","center":a.center().to_array(),"radius":a.radius(),"sweep":a.sweep_radians(),
                "midpoint":a.point_at(0.5)?.to_array(),"end_midpoint":midpoint,
                "half_chord_tensor":outer(chord)?,"plane_tensor":outer(a.normal()?.as_vector().to_array())?})
        }
        _ => {
            return Err(ProbeError::FixtureInvariant(
                "unexpected curvature marker geometry",
            ));
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marker_records_keep_finite_centers_and_reject_tensor_overflow() {
        let line = |a, b| {
            Geometry::Line(
                LineSegment::try_new(
                    Point3::try_from(a).unwrap(),
                    Point3::try_from(b).unwrap(),
                    Tolerance::DEFAULT,
                )
                .unwrap(),
            )
        };
        let value = marker(&line([1e308, -1.0, 0.0], [1e308, 1.0, 0.0])).unwrap();
        assert_eq!(value["center"], json!([1e308, 0.0, 0.0]));
        assert_eq!(value["half_chord_tensor"][1][1], json!(1.0));
        assert!(marker(&line([0.0; 3], [1e200, 0.0, 0.0])).is_err());
    }
}
