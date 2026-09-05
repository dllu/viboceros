//! Affinely independent witnesses for actual construction-plane commands.
use super::*;
use viboceros_command::CommandContext;

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct PlaneTransformFixture {
    pub origin: [f64; 3],
    pub x_axis: [f64; 3],
    pub y_axis: [f64; 3],
    pub command: String,
    pub references: Vec<[f64; 3]>,
    pub value: Option<f64>,
    pub copy: bool,
    pub sources: Vec<[f64; 3]>,
}

pub(super) fn run(
    f: &PlaneTransformFixture,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    let plane = Frame3::try_from_directions(
        Point3::try_from(f.origin)?,
        Vector3::try_from(f.x_axis)?,
        Vector3::try_from(f.y_axis)?,
        tolerance,
    )?;
    let expected = match f.command.as_str() {
        "Rotate" | "Scale2D" => {
            if f.value.is_some() {
                1
            } else {
                3
            }
        }
        "Shear" => {
            if f.value.is_some() {
                2
            } else {
                3
            }
        }
        "Mirror" if f.value.is_none() => 2,
        "ProjectToCPlane" if f.value.is_none() => 0,
        _ => return Err(ProbeError::FixtureInvariant("unsupported plane transform")),
    };
    if f.references.len() != expected || f.sources.is_empty() || f.sources.len() > 256 {
        return Err(ProbeError::FixtureInvariant(
            "incorrect transform fixture arguments",
        ));
    }
    let mut command = f.command.clone();
    for p in &f.references {
        Point3::try_from(*p)?;
        command.push_str(&format!(" {},{},{}", p[0], p[1], p[2]));
    }
    if let Some(value) = f.value {
        if !value.is_finite() {
            return Err(ProbeError::FixtureInvariant("nonfinite transform value"));
        }
        command.push_str(&format!(" {value}"));
    }
    command.push_str(&if f.command == "ProjectToCPlane" {
        format!(" DeleteInput={}", if f.copy { "No" } else { "Yes" })
    } else {
        format!(" Copy={}", if f.copy { "Yes" } else { "No" })
    });
    let mut document = Document::new(tolerance);
    let registry = CommandRegistry::with_builtins();
    let mut source_ids = Vec::new();
    for (index, p) in f.sources.iter().enumerate() {
        let id = document.add_geometry(Geometry::Point(Point3::try_from(*p)?))?;
        document.set_object_names([(id, Some(index.to_string()))])?;
        source_ids.push(id);
    }
    registry.execute(&mut document, "SelAll")?;
    registry.execute_in_context(
        &mut document,
        &command,
        CommandContext {
            construction_plane: plane,
        },
    )?;
    let mut records = document.objects().map(|object| {
        let Geometry::Point(point) = object.geometry() else { unreachable!("point transform") };
        let index: usize = object.attributes().name().unwrap().parse().unwrap();
        let original = source_ids.contains(&object.id());
        ((index, !original), json!({"source":index,"point":point.to_array(),"original":original,"selected":document.is_selected(object.id())}))
    }).collect::<Vec<_>>();
    records.sort_by_key(|(key, _)| *key);
    Ok((
        json!({"objects":records.into_iter().map(|(_, record)| record).collect::<Vec<_>>()}),
        0,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn permanent_plane_transform_fixtures_span_3d_and_preserve_source_state() {
        for data in [
            include_str!("../../../tools/rhino_oracle/fixtures/plane_transforms.json"),
            include_str!("../../../tools/rhino_oracle/fixtures/plane_transform_diagnostics.json"),
        ] {
            let request: ProbeRequest = serde_json::from_str(data).unwrap();
            let response = run_request(&request).unwrap();
            for (operation, result) in request.operations.iter().zip(&response.results) {
                let Operation::PlaneTransform { fixture, .. } = operation else {
                    panic!("transform fixture")
                };
                let p = &fixture.sources;
                assert_eq!(p.len(), 4);
                let start = Point3::try_from(p[0]).unwrap();
                let vectors =
                    [1, 2, 3].map(|i| start.vector_to(Point3::try_from(p[i]).unwrap()).unwrap());
                assert!(
                    vectors[0]
                        .dot(vectors[1].cross(vectors[2]).unwrap())
                        .unwrap()
                        .abs()
                        > 30.0
                );
                let objects = result.value["objects"].as_array().unwrap();
                assert_eq!(objects.len(), if fixture.copy { 8 } else { 4 });
                for (index, source) in p.iter().enumerate() {
                    let original = &objects[index * if fixture.copy { 2 } else { 1 }];
                    assert_eq!(original["source"], index);
                    assert_eq!(original["original"], true);
                    assert_eq!(original["selected"], true);
                    if fixture.copy {
                        assert_eq!(original["point"], json!(source));
                        assert_eq!(objects[index * 2 + 1]["original"], false);
                        assert_eq!(objects[index * 2 + 1]["selected"], false);
                    }
                }
                assert_eq!(result.elapsed_ns, 0);
            }
        }
    }
}
