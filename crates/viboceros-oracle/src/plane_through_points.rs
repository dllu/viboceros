//! Complete command reports; oriented tensor differences remain observable.
use super::*;
use viboceros_command::CommandContext;
use viboceros_document::{ColorRgb, ControlPointId, ObjectAttributes, SelectionMode};

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    Points,
    Cloud,
    MeshGrips,
    CurveGrips,
}
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Fixture {
    pub points: Vec<[f64; 3]>,
    pub source: Source,
    pub postselect: bool,
}
pub(super) fn run(
    f: &Fixture,
    iterations: u32,
    tolerance: Tolerance,
) -> Result<(Value, u64), ProbeError> {
    if !(1..=256).contains(&f.points.len())
        || (matches!(f.source, Source::MeshGrips | Source::CurveGrips)
            && (f.points.len() < 3 || f.postselect))
    {
        return Err(ProbeError::FixtureInvariant(
            "invalid PlaneThroughPt sources",
        ));
    }
    let points = f
        .points
        .iter()
        .map(|p| point(*p))
        .collect::<Result<Vec<_>, _>>()?;
    measure_probe(iterations, || -> Result<Value, ProbeError> {
        let mut doc = Document::new(tolerance);
        let source_layer = doc.add_layer("PlaneFitSource", ColorRgb::new(255, 0, 0))?;
        let current_layer = doc.add_layer("PlaneFitCurrent", ColorRgb::new(0, 0, 255))?;
        doc.set_current_layer(current_layer)?;
        let geometries = match f.source {
            Source::Points => points
                .iter()
                .copied()
                .map(Geometry::Point)
                .collect::<Vec<_>>(),
            Source::Cloud => vec![Geometry::PointCloud(PointCloud3::try_new(points.clone())?)],
            Source::MeshGrips => vec![Geometry::Mesh(TriangleMesh::try_new(
                points.clone(),
                vec![[0, 1, 2]],
                tolerance,
            )?)],
            Source::CurveGrips => vec![Geometry::NurbsCurve(NurbsCurve::try_clamped_uniform(
                1,
                points.clone(),
            )?)],
        };
        let ids = geometries
            .into_iter()
            .map(|g| {
                doc.add_geometry_with_attributes(
                    g,
                    ObjectAttributes::on_layer(source_layer)
                        .with_name("fit-source")
                        .with_object_color(ColorRgb::new(255, 0, 0)),
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        if matches!(f.source, Source::MeshGrips | Source::CurveGrips) {
            doc.enable_control_points(ids.iter().copied())?;
            doc.select_control_points(
                (0..points.len()).map(|index| ControlPointId {
                    object: ids[0],
                    index,
                }),
                SelectionMode::Replace,
            )?;
        } else {
            doc.select_objects_direct(ids.iter().copied(), SelectionMode::Replace)?;
        }
        let before = doc.objects().cloned().collect::<Vec<_>>();
        let registry = CommandRegistry::with_builtins();
        let result = if f.postselect {
            registry.execute_postselected(&mut doc, "PlaneThroughPt", CommandContext::default())
        } else {
            registry.execute(&mut doc, "PlaneThroughPt")
        };
        let succeeded = match result {
            Ok(_) => true,
            Err(viboceros_command::CommandError::OperationDeclined) => false,
            Err(_) if points.len() < 3 => false,
            Err(error) => return Err(error.into()),
        };
        let mut outputs = vec![];
        for obj in doc.objects().filter(|o| !ids.contains(&o.id())) {
            let Geometry::NurbsSurface(surface) = obj.geometry() else {
                return Err(ProbeError::FixtureInvariant(
                    "PlaneThroughPt produced a non-surface",
                ));
            };
            let corners = [(0_f64, 0_f64), (1., 0.), (0., 1.), (1., 1.)]
                .into_iter()
                .map(|(u, v)| {
                    surface
                        .evaluate(
                            u.mul_add(
                                surface.domain_u().end() - surface.domain_u().start(),
                                *surface.domain_u().start(),
                            ),
                            v.mul_add(
                                surface.domain_v().end() - surface.domain_v().start(),
                                *surface.domain_v().start(),
                            ),
                        )
                        .map(|p| p.to_array())
                })
                .collect::<Result<Vec<_>, _>>()?;
            outputs.push(json!({"definition":nurbs_surface_definition_value(surface),"corners":corners,
                "layer":doc.layer(obj.attributes().layer_id()).unwrap().name(),"name":obj.attributes().name(),
                "selected":doc.is_selected(obj.id()),"groups":obj.group_ids().iter().map(|g|g.to_string()).collect::<Vec<_>>(),
                "color_source":if obj.attributes().color_source()==viboceros_document::ObjectColorSource::Layer {"ColorFromLayer"}else{"ColorFromObject"}}));
        }
        let mut sources = vec![];
        for (i, &id) in ids.iter().enumerate() {
            let obj = doc.object(id).ok_or(ProbeError::FixtureInvariant(
                "PlaneThroughPt consumed a source",
            ))?;
            if obj.geometry() != before[i].geometry() || obj.attributes() != before[i].attributes()
            {
                return Err(ProbeError::FixtureInvariant(
                    "PlaneThroughPt changed a source",
                ));
            }
            sources.push(json!({"present":true,"selected":doc.is_selected(id),"grips_selected":doc.selected_control_points().filter(|(p,_)|p.object==id).count()}));
        }
        Ok(json!({"command_returned":succeeded,"outputs":outputs,"sources":sources}))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn plane_through_points_protocol_retains_geometry_metadata_and_raw_chart_differences() {
        let request: ProbeRequest = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/plane_through_points.json"
        ))
        .unwrap();
        let native: Value = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/observations/plane_through_points_rhino_20261016.json"
        ))
        .unwrap();
        let response = run_request(&request).unwrap();
        assert_eq!(response.results.len(), 17);
        for (row, reference) in response
            .results
            .iter()
            .zip(native["results"].as_array().unwrap())
        {
            assert_eq!(row.id, reference["id"].as_str().unwrap());
            assert_eq!(row.value["sources"], reference["value"]["sources"]);
            assert_eq!(
                row.value["command_returned"],
                reference["value"]["command_returned"]
            );
            let outputs = row.value["outputs"].as_array().unwrap();
            let expected = reference["value"]["outputs"].as_array().unwrap();
            assert_eq!(outputs.len(), expected.len());
            for (actual, expected) in outputs.iter().zip(expected) {
                for field in ["layer", "name", "groups", "color_source", "selected"] {
                    assert_eq!(actual[field], expected[field]);
                }
                let a = actual["corners"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|p| point(serde_json::from_value(p.clone()).unwrap()).unwrap())
                    .collect::<Vec<_>>();
                let b = expected["corners"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|p| point(serde_json::from_value(p.clone()).unwrap()).unwrap())
                    .collect::<Vec<_>>();
                let mut matched = false;
                for swap in [false, true] {
                    for flip_u in [false, true] {
                        for flip_v in [false, true] {
                            matched |= a.iter().enumerate().all(|(i, a)| {
                                let (u, v) = if swap { (i / 2, i % 2) } else { (i % 2, i / 2) };
                                let j = (if flip_u { 1 - u } else { u })
                                    + 2 * (if flip_v { 1 - v } else { v });
                                a.distance_bound_to(b[j], 1e-6).unwrap().is_some()
                            });
                        }
                    }
                }
                assert!(matched, "{}", row.id);
            }
        }
    }
}
