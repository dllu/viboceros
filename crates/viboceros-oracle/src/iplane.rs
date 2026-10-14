//! IPlane command outputs for comparison with public Rhino plane intersection APIs.
use super::*;
use crate::object_source::ObjectSource;

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Fixture {
    pub sources: Vec<ObjectSource>,
    pub origin: [f64; 3],
    pub normal: [f64; 3],
}
pub(super) fn run(
    f: &Fixture,
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    if !(1..=64).contains(&f.sources.len()) {
        return Err(ProbeError::FixtureInvariant(
            "IPlane requires 1..64 sources",
        ));
    }
    let mut source = Document::new(tolerance);
    for definition in &f.sources {
        let geometry = definition.geometry(tolerance)?;
        let id = source.add_geometry(geometry)?;
        if !viboceros_command::ObjectSelectionFilter::Section
            .accepts_object(source.object(id).unwrap())
        {
            return Err(ProbeError::FixtureInvariant("unsupported IPlane source"));
        }
    }
    source.select_objects_direct(
        source.objects().map(|o| o.id()).collect::<Vec<_>>(),
        SelectionMode::Replace,
    )?;
    let registry = CommandRegistry::with_builtins();
    let command = format!(
        "IPlane Plane={},{},{},{},{},{}",
        f.origin[0], f.origin[1], f.origin[2], f.normal[0], f.normal[1], f.normal[2]
    );
    measure_probe(iterations, || {
        let mut document = source.clone();
        registry.execute(&mut document, &command)?;
        let mut points = Vec::new();
        let mut curves = Vec::new();
        for object in document.selected_objects() {
            match object.geometry() {
                Geometry::Point(point) => points.push(point.to_array()),
                geometry => {
                    let curve = geometry
                        .curve_ref()
                        .ok_or(ProbeError::FixtureInvariant("unsupported IPlane result"))?;
                    let mut samples = curve
                        .divide_by_count(32, true, tolerance)?
                        .into_iter()
                        .map(Point3::to_array)
                        .collect::<Vec<_>>();
                    if compare_point(samples.last().unwrap(), &samples[0]).is_lt() {
                        samples.reverse();
                    }
                    curves.push(json!({"samples":samples,"length":curve.length(tolerance)?,"closed":curve.is_closed()?}));
                }
            }
        }
        points.sort_by(compare_point);
        curves.sort_by(|a, b| {
            let point = |v: &Value| std::array::from_fn(|i| v["samples"][0][i].as_f64().unwrap());
            compare_point(&point(a), &point(b))
        });
        Ok::<_, ProbeError>(json!({"points":points,"curves":curves}))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn plane_intersection_recipes_cover_points_misses_overlaps_surfaces_meshes_and_breps() {
        let request: ProbeRequest = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/iplane.json"
        ))
        .unwrap();
        let result = run_request(&request).unwrap();
        assert_eq!(result.results.len(), 6);
        let value = |i: usize| &result.results[i].value;
        assert_eq!(value(0)["points"], json!([[10., 20., 0.]]));
        assert_eq!(value(1), &json!({"points":[],"curves":[]}));
        for i in 2..6 {
            assert_eq!(
                value(i)["curves"].as_array().unwrap().len(),
                1,
                "{}",
                result.results[i].id
            );
            for p in value(i)["curves"][0]["samples"].as_array().unwrap() {
                assert!(p[2].as_f64().unwrap().abs() <= request.tolerance.absolute);
            }
        }
    }
}
