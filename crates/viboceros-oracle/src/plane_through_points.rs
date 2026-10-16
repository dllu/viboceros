//! Complete command reports; oriented tensor differences remain observable.
use super::*;
use viboceros_command::CommandContext;
use viboceros_command::point_fitting::PointFitKind;
use viboceros_document::{ColorRgb, ControlPointId, ObjectAttributes, SelectionMode};

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    Points,
    Cloud,
    Mesh,
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
    run_kind(f, iterations, tolerance, PointFitKind::Plane)
}

pub(super) fn run_kind(
    f: &Fixture,
    iterations: u32,
    tolerance: Tolerance,
    kind: PointFitKind,
) -> Result<(Value, u64), ProbeError> {
    if !(1..=256).contains(&f.points.len())
        || (f.source == Source::Mesh && (kind == PointFitKind::Plane || f.points.len() < 3))
        || (matches!(f.source, Source::MeshGrips | Source::CurveGrips) && f.postselect)
        || (f.source == Source::MeshGrips && f.points.len() < 3)
        || (f.source == Source::CurveGrips && f.points.len() < kind.minimum_points())
    {
        return Err(ProbeError::FixtureInvariant(
            "invalid point fitting sources",
        ));
    }
    let points = f
        .points
        .iter()
        .map(|p| point(*p))
        .collect::<Result<Vec<_>, _>>()?;
    measure_probe(iterations, || -> Result<Value, ProbeError> {
        let mut doc = Document::new(tolerance);
        let prefix = match kind {
            PointFitKind::Plane => "PlaneFit",
            PointFitKind::Line => "LineFit",
            PointFitKind::Sphere => "SphereFit",
        };
        let source_layer = doc.add_layer(format!("{prefix}Source"), ColorRgb::new(255, 0, 0))?;
        let current_layer = doc.add_layer(format!("{prefix}Current"), ColorRgb::new(0, 0, 255))?;
        doc.set_current_layer(current_layer)?;
        let geometries = match f.source {
            Source::Points => points
                .iter()
                .copied()
                .map(Geometry::Point)
                .collect::<Vec<_>>(),
            Source::Cloud => vec![Geometry::PointCloud(PointCloud3::try_new(points.clone())?)],
            Source::MeshGrips | Source::Mesh => vec![Geometry::Mesh(TriangleMesh::try_new(
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
        let incomplete_getter =
            kind == PointFitKind::Sphere && kind.selected_point_count(&doc) < kind.minimum_points();
        if incomplete_getter {
            doc.clear_selection();
        }
        let result = if incomplete_getter {
            Err(viboceros_command::CommandError::OperationDeclined)
        } else if f.postselect {
            registry.execute_postselected(&mut doc, kind.name(), CommandContext::default())
        } else {
            registry.execute(&mut doc, kind.name())
        };
        let succeeded = match result {
            Ok(_) => true,
            Err(viboceros_command::CommandError::OperationDeclined) => false,
            Err(_) if points.len() < kind.minimum_points() => false,
            Err(error) => return Err(error.into()),
        };
        let mut outputs = vec![];
        for obj in doc.objects().filter(|o| !ids.contains(&o.id())) {
            let mut value = match (obj.geometry(), kind) {
                (Geometry::Line(line), PointFitKind::Line) => {
                    json!({"definition":nurbs_curve_definition_value(&line.to_nurbs()?),"endpoints":[line.start().to_array(),line.end().to_array()]})
                }
                (Geometry::NurbsSurface(surface), PointFitKind::Plane) => {
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
                    json!({"definition":nurbs_surface_definition_value(surface),"corners":corners})
                }
                (Geometry::NurbsSurface(surface), PointFitKind::Sphere) => {
                    let fit = viboceros_geometry::SphereFit::try_fit_to_points(&points)?
                        .ok_or(ProbeError::FixtureInvariant("missing sphere parameters"))?;
                    json!({"center":fit.center().to_array(),"radius":fit.radius(),"definition":nurbs_surface_definition_value(surface)})
                }
                _ => {
                    return Err(ProbeError::FixtureInvariant(
                        "point fitting produced unexpected geometry",
                    ));
                }
            };
            value["layer"] = json!(doc.layer(obj.attributes().layer_id()).unwrap().name());
            value["name"] = json!(obj.attributes().name());
            value["selected"] = json!(doc.is_selected(obj.id()));
            value["groups"] = json!(
                obj.group_ids()
                    .iter()
                    .map(|g| g.to_string())
                    .collect::<Vec<_>>()
            );
            value["color_source"] = json!(if obj.attributes().color_source()
                == viboceros_document::ObjectColorSource::Layer
            {
                "ColorFromLayer"
            } else {
                "ColorFromObject"
            });
            outputs.push(value);
        }
        let mut sources = vec![];
        for (i, &id) in ids.iter().enumerate() {
            let obj = doc.object(id).ok_or(ProbeError::FixtureInvariant(
                "point fitting consumed a source",
            ))?;
            if obj.geometry() != before[i].geometry() || obj.attributes() != before[i].attributes()
            {
                return Err(ProbeError::FixtureInvariant(
                    "point fitting changed a source",
                ));
            }
            sources.push(json!({"present":true,"selected":doc.is_selected(id),"grips_selected":doc.selected_control_points().filter(|(p,_)|p.object==id).count()}));
        }
        let mut value = json!({"command_returned":succeeded,"outputs":outputs,"sources":sources});
        if kind == PointFitKind::Line {
            let fit = if points.len() < 2 {
                None
            } else {
                LineSegment::try_fit_to_points(&points)?
            };
            value["sdk"] = json!({"created":fit.is_some(),"endpoints":fit.map(|l|vec![l.start().to_array(),l.end().to_array()]).unwrap_or_default()});
        }
        if kind == PointFitKind::Sphere {
            let fit = if points.len() < 3 {
                None
            } else {
                viboceros_geometry::SphereFit::try_fit_to_points(&points)?
            };
            value["sdk"] = json!({"created":fit.is_some(),"center":fit.map(|f|f.center().to_array()),"radius":fit.map(|f|f.radius())});
        }
        Ok(value)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sphere_fit_points_protocol_retains_native_cap_difference_and_getter_cancellation() {
        let request: ProbeRequest = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/sphere_fit_points.json"
        ))
        .unwrap();
        let native: Value = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/observations/sphere_fit_points_rhino_20261016.json"
        ))
        .unwrap();
        let actual = run_request(&request).unwrap();
        assert_eq!(actual.results.len(), 21);
        let mut images = 0;
        let mut mismatches = 0;
        for (row, expected) in actual
            .results
            .iter()
            .zip(native["results"].as_array().unwrap())
        {
            let expected = &expected["value"];
            assert_eq!(
                row.value["command_returned"], expected["command_returned"],
                "{}",
                row.id
            );
            assert_eq!(row.value["sources"], expected["sources"], "{}", row.id);
            assert_eq!(
                row.value["outputs"].as_array().unwrap().len(),
                expected["outputs"].as_array().unwrap().len()
            );
            for (a, b) in row.value["outputs"]
                .as_array()
                .unwrap()
                .iter()
                .zip(expected["outputs"].as_array().unwrap())
            {
                for key in ["layer", "name", "groups", "selected", "color_source"] {
                    assert_eq!(a[key], b[key]);
                }
                let center = point(serde_json::from_value(a["center"].clone()).unwrap()).unwrap();
                let reference =
                    point(serde_json::from_value(b["center"].clone()).unwrap()).unwrap();
                let error = center.distance_to(reference).unwrap()
                    + (a["radius"].as_f64().unwrap() - b["radius"].as_f64().unwrap()).abs();
                if row.id == "shallow-cap" {
                    assert!(error > 1.);
                    mismatches += 1;
                } else {
                    assert!(error <= 1e-6, "{} {error}", row.id);
                    images += 1;
                }
            }
        }
        assert_eq!((images, mismatches), (15, 1));
    }
    #[test]
    fn point_fitting_protocol_replays_line_sdk_images_and_mixed_native_command_states() {
        let request: ProbeRequest = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/fixtures/point_fitting.json"
        ))
        .unwrap();
        let native: Value = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/observations/point_fitting_rhino_20261016.json"
        ))
        .unwrap();
        let response = run_request(&request).unwrap();
        assert_eq!(response.results.len(), 42);
        let mut lines = 0;
        for (row, expected) in response
            .results
            .iter()
            .zip(native["results"].as_array().unwrap())
        {
            assert_eq!(row.id, expected["id"].as_str().unwrap());
            let actual = &row.value;
            let expected = &expected["value"];
            assert_eq!(actual["command_returned"], expected["command_returned"]);
            assert_eq!(actual["sources"], expected["sources"]);
            assert_eq!(
                actual["outputs"].as_array().unwrap().len(),
                expected["outputs"].as_array().unwrap().len()
            );
            if row.id.starts_with("line-") {
                assert_eq!(actual["sdk"]["created"], expected["sdk"]["created"]);
                for key in ["sdk", "command"] {
                    let (a, b) = if key == "sdk" {
                        (&actual["sdk"], &expected["sdk"])
                    } else if actual["outputs"].as_array().unwrap().is_empty() {
                        continue;
                    } else {
                        (&actual["outputs"][0], &expected["outputs"][0])
                    };
                    let a = a["endpoints"].as_array().unwrap();
                    let b = b["endpoints"].as_array().unwrap();
                    assert_eq!(a.len(), b.len());
                    if a.is_empty() {
                        continue;
                    }
                    assert!(
                        [false, true].into_iter().any(|reverse| (0..2).all(|i| {
                            let a = point(serde_json::from_value(a[i].clone()).unwrap()).unwrap();
                            let b = point(
                                serde_json::from_value(b[if reverse { 1 - i } else { i }].clone())
                                    .unwrap(),
                            )
                            .unwrap();
                            a.distance_bound_to(b, 1e-6).unwrap().is_some()
                        })),
                        "{} {key}",
                        row.id
                    );
                }
                lines += 1;
            }
            for (a, b) in actual["outputs"]
                .as_array()
                .unwrap()
                .iter()
                .zip(expected["outputs"].as_array().unwrap())
            {
                for key in ["layer", "name", "groups", "color_source", "selected"] {
                    assert_eq!(a[key], b[key]);
                }
            }
        }
        assert_eq!(lines, 25);
        let two_grips = Fixture {
            points: vec![[0., 0., 0.], [1., 2., 3.]],
            source: Source::CurveGrips,
            postselect: false,
        };
        let (value, _) = run_kind(&two_grips, 1, Tolerance::DEFAULT, PointFitKind::Line).unwrap();
        assert_eq!(value["outputs"].as_array().unwrap().len(), 1);
        assert_eq!(value["sources"][0]["grips_selected"], 2);
    }
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
