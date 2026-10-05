//! Closed independently constructed recipes for the Python comparison API.
use super::*;
use viboceros_geometry::{BoundingBox3, BrepBooleanOperation, BrepSolidOrientation, Real};

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct PolyhedralBooleanFixture {
    case: String,
    operation: String,
}

pub(super) fn run(
    f: &PolyhedralBooleanFixture,
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    if iterations != 1 {
        return Err(ProbeError::FixtureInvariant(
            "polyhedral Boolean recipes require one iteration",
        ));
    }
    let operation = match f.operation.as_str() {
        "union" => BrepBooleanOperation::Union,
        "intersection" => BrepBooleanOperation::Intersection,
        "difference" => BrepBooleanOperation::Difference,
        _ => {
            return Err(ProbeError::FixtureInvariant(
                "unknown polyhedral Boolean operation",
            ));
        }
    };
    let (a, b) = source(&f.case, tolerance)?;
    let inputs = vec![record(&a, tolerance)?, record(&b, tolerance)?];
    let timer = Instant::now();
    let result = viboceros_geometry::boolean_polyhedral_breps(&a, &b, operation, tolerance);
    let elapsed = u64::try_from(timer.elapsed().as_nanos()).unwrap_or(u64::MAX);
    let (outputs, error) = match result {
        Ok(bodies) => (
            bodies
                .into_iter()
                .map(|b| record(&b.brep, tolerance))
                .collect::<Result<Vec<_>, _>>()?,
            None,
        ),
        Err(error @ GeometryError::UnrepresentableBrepBoolean) => {
            (Vec::new(), Some(error.to_string()))
        }
        Err(error) => return Err(error.into()),
    };
    Ok((
        json!({"inputs":inputs,"returned_null":false,"outputs":outputs,"kernel_error":error}),
        elapsed,
    ))
}

fn source(case: &str, tolerance: Tolerance) -> Result<(Brep, Brep), ProbeError> {
    let frame = Frame3::try_from_directions(
        Point3::try_new(0., 0., 0.)?,
        Vector3::try_new(1., 0., 0.)?,
        Vector3::try_new(0., 1., 0.)?,
        tolerance,
    )?;
    let cube = |bounds| Brep::try_box(frame, bounds, tolerance);
    let one = |a: &Brep, b: &Brep, op| -> Result<Brep, ProbeError> {
        let brep = a
            .try_boolean_convex(b, op, tolerance)?
            .ok_or(ProbeError::FixtureInvariant(
                "polyhedral source must be nonempty",
            ))?;
        let groups = vec![0; brep.faces().len()];
        let merged = brep
            .try_merge_coplanar_polygon_faces_in_groups(&groups, tolerance)?
            .unwrap_or(brep);
        Ok(merged.try_merge_all_edges(0., tolerance)?)
    };
    let mut a = cube([[0., 3.]; 3])?;
    let mut b = cube([[1.5, 3.5], [1.5, 3.5], [1., 2.]])?;
    match case {
        "concave" | "coplanar_concave" => {
            a = one(
                &a,
                &cube([[2., 4.], [1., 2.], [0., 3.]])?,
                BrepBooleanOperation::Union,
            )?;
            if case == "coplanar_concave" {
                b = cube([[2., 4.], [1., 3.], [0., 3.]])?;
            }
        }
        "hole" | "reversed_hole" | "two_holes" | "singular_two_holes" => {
            a = one(
                &a,
                &cube([[1., 2.], [1., 2.], [-1., 4.]])?,
                BrepBooleanOperation::Difference,
            )?;
            if case == "reversed_hole" {
                a = a.reversed();
            }
            if case == "two_holes" || case == "singular_two_holes" {
                let interval = if case == "two_holes" {
                    [2.25, 3.25]
                } else {
                    [2., 3.]
                };
                b = one(
                    &cube([[1., 4.]; 3])?,
                    &cube([interval, interval, [0., 5.]])?,
                    BrepBooleanOperation::Difference,
                )?;
            }
        }
        "cavity" => {
            a = Brep::try_disjoint_union(vec![a, cube([[1., 2.]; 3])?.reversed()], tolerance)?;
        }
        "island" => {
            a = Brep::try_disjoint_union(
                vec![
                    cube([[0., 4.]; 3])?,
                    cube([[1., 3.]; 3])?.reversed(),
                    cube([[1.5, 2.5]; 3])?,
                ],
                tolerance,
            )?;
            b = cube([[2., 5.]; 3])?;
        }
        "disjoint_shells" => {
            a = Brep::try_disjoint_union(vec![a, cube([[4., 5.]; 3])?], tolerance)?;
            b = cube([[2., 4.5]; 3])?;
        }
        "rounded_uv" => {
            a = one(
                &a,
                &cube([[0.5, 2.5], [-1., 4.], [-1., 4.]])?,
                BrepBooleanOperation::Intersection,
            )?;
        }
        _ => {
            return Err(ProbeError::FixtureInvariant(
                "unknown polyhedral Boolean source recipe",
            ));
        }
    }
    Ok((a, b))
}

fn record(brep: &Brep, tolerance: Tolerance) -> Result<Value, ProbeError> {
    let mass = brep.volume_mass_properties(tolerance)?;
    let bounds = BoundingBox3::from_points(brep.vertices().iter().map(|v| v.point()))?;
    let faces = brep.faces().iter().map(|f| json!({"loops": f.loops().iter().map(|l|l.trims().iter().map(|t|brep.vertices()[t.vertices()[0]].point().to_array()).collect::<Vec<_>>()).collect::<Vec<_>>() })).collect::<Vec<_>>();
    let edges = brep
        .edges()
        .iter()
        .map(|e| {
            let domain = e.curve().domain();
            (0..=8)
                .map(|i| {
                    e.curve()
                        .evaluate(
                            *domain.start() + (*domain.end() - *domain.start()) * i as Real / 8.,
                        )
                        .map(|p| p.to_array())
                })
                .collect::<Result<Vec<_>, _>>()
        })
        .collect::<Result<Vec<_>, _>>()?;
    let orientation = match brep.solid_orientation()? {
        BrepSolidOrientation::Outward => "Outward",
        BrepSolidOrientation::Inward => "Inward",
        BrepSolidOrientation::NotSolid => "None",
        BrepSolidOrientation::Unknown => "Unknown",
    };
    Ok(
        json!({"valid":true,"solid":brep.is_solid(),"orientation":orientation,
        "volume":mass.signed_volume()?.abs(),"centroid":mass.centroid()?.to_array(),"area":brep.area(tolerance)?,
        "bounds":[bounds.min().to_array(),bounds.max().to_array()],"faces":faces,
        "vertices":brep.vertices().iter().map(|v|v.point().to_array()).collect::<Vec<_>>(),"edge_samples":edges}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn python_protocol_covers_all_closed_recipes_and_retains_singular_rejections() {
        let request: ProbeRequest = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/polyhedral_boolean.json"
        ))
        .unwrap();
        let response = run_request(&request).unwrap();
        assert_eq!(response.results.len(), 30);
        let errors = response
            .results
            .iter()
            .filter(|r| !r.value["kernel_error"].is_null())
            .map(|r| r.id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(errors, ["polyhedral_singular_two_holes_intersection"]);
    }
}
