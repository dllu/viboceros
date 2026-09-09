//! Geometry-filtered selection commands, independent of interactive picking.

use super::*;

pub(super) struct SelectGeometryCommand {
    pub(super) name: &'static str,
    pub(super) aliases: &'static [&'static str],
    pub(super) filter: GeometrySelectionFilter,
}

impl Command for SelectGeometryCommand {
    fn name(&self) -> &'static str {
        self.name
    }

    fn aliases(&self) -> &'static [&'static str] {
        self.aliases
    }

    fn records_history(&self) -> bool {
        false
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        require_consumed(arguments, 0, self.name)?;
        let tolerance = document.tolerance();
        select_matching_geometry(document, |geometry| {
            self.filter.matches(geometry, tolerance)
        })
    }
}

/// Retain only matches, but defer every selection change until all fallible
/// predicates succeed. Hidden/locked objects never reach the predicate.
fn select_matching_geometry(
    document: &mut Document,
    mut predicate: impl FnMut(&Geometry) -> Result<bool, GeometryError>,
) -> Result<String, CommandError> {
    let mut matches = Vec::new();
    for object in document.selectable_objects() {
        if predicate(object.geometry())? {
            matches.push(object.id());
        }
    }
    document.select_objects(matches, SelectionMode::Add)?;
    Ok(format!(
        "Selected {} object(s)",
        document.selected_object_count()
    ))
}

#[derive(Clone, Copy)]
pub(super) enum GeometrySelectionFilter {
    Curve,
    OpenCurve,
    ClosedCurve,
    PlanarCurve,
    Line,
    Polyline,
    Point,
    PointCloud,
    Surface,
    Polysurface,
    OpenPolysurface,
    ClosedPolysurface,
    Mesh,
    OpenMesh,
    ClosedMesh,
    NonManifold,
}

impl GeometrySelectionFilter {
    fn matches(self, geometry: &Geometry, tolerance: Tolerance) -> Result<bool, GeometryError> {
        let matches = match self {
            Self::Curve => geometry_curve_ref(geometry).is_some(),
            Self::OpenCurve => match geometry_curve_ref(geometry) {
                Some(curve) => !curve.is_closed()?,
                None => false,
            },
            Self::ClosedCurve => match geometry_curve_ref(geometry) {
                Some(curve) => curve.is_closed()?,
                None => false,
            },
            Self::PlanarCurve => match geometry_curve_ref(geometry) {
                Some(curve) => curve.is_planar(tolerance)?,
                None => false,
            },
            Self::Line => match geometry {
                Geometry::Line(_) => true,
                Geometry::NurbsCurve(curve) => {
                    curve.spans().count() == 1 && curve.is_linear_at_zero_tolerance()?
                }
                _ => false,
            },
            Self::Polyline => match geometry {
                Geometry::Polyline(_) => true,
                Geometry::NurbsCurve(curve) => {
                    curve.degree() == 1 && curve.control_points().len() > 2
                }
                _ => false,
            },
            Self::Point => matches!(geometry, Geometry::Point(_)),
            Self::PointCloud => matches!(geometry, Geometry::PointCloud(_)),
            Self::Surface => match geometry {
                Geometry::NurbsSurface(_) => true,
                Geometry::Brep(brep) => brep.faces().len() == 1,
                _ => false,
            },
            Self::Polysurface => match geometry {
                Geometry::Brep(brep) => brep.faces().len() > 1,
                _ => false,
            },
            Self::OpenPolysurface => match geometry {
                Geometry::Brep(brep) => brep.faces().len() > 1 && !brep.is_closed(),
                _ => false,
            },
            Self::ClosedPolysurface => match geometry {
                Geometry::Brep(brep) => brep.faces().len() > 1 && brep.is_closed(),
                _ => false,
            },
            Self::Mesh => matches!(geometry, Geometry::Mesh(_)),
            Self::OpenMesh => match geometry {
                Geometry::Mesh(mesh) => !mesh.topology().is_closed(),
                _ => false,
            },
            Self::ClosedMesh => match geometry {
                Geometry::Mesh(mesh) => mesh.topology().is_closed(),
                _ => false,
            },
            Self::NonManifold => match geometry {
                Geometry::Mesh(mesh) => !mesh.topology().is_manifold(),
                Geometry::Brep(brep) => !brep.is_manifold(),
                _ => false,
            },
        };
        Ok(matches)
    }
}

pub(super) struct SelShortCurveCommand;

impl Command for SelShortCurveCommand {
    fn name(&self) -> &'static str {
        "SelShortCrv"
    }

    fn records_history(&self) -> bool {
        false
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let [maximum_length] = arguments else {
            return Err(CommandError::Usage("SelShortCrv maximum-length"));
        };
        let maximum_length = parse_positive_curve_length(maximum_length)?;
        // Rhino's command includes a relative 1e-6 allowance, including its
        // boundary but not the next float (see the retained line probes).
        // Above MAX every representable length is eligible; keep a finite cap.
        let comparison_limit = (maximum_length * 1.000001).min(Real::MAX);
        select_matching_geometry(document, |geometry| match geometry_curve_ref(geometry) {
            Some(curve) => curve.is_short_for_selection(comparison_limit),
            None => Ok(false),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    #[test]
    #[ignore = "manual geometry-selection timing"]
    fn benchmark_large_geometry_selection() {
        let mut document = Document::default();
        document.begin_transaction("fixture").unwrap();
        let ids = (0..20_000)
            .map(|x| {
                document
                    .add_geometry(Geometry::Point(Point3::try_new(x as f64, 0., 0.).unwrap()))
                    .unwrap()
            })
            .collect::<BTreeSet<_>>();
        document.commit_transaction().unwrap();
        let start = std::time::Instant::now();
        assert_eq!(
            select_matching_geometry(&mut document, |_| Ok(true)).unwrap(),
            "Selected 20000 object(s)"
        );
        eprintln!("20k geometry selection: {:?}", start.elapsed());
        assert_eq!(document.selected_object_ids().collect::<BTreeSet<_>>(), ids);
    }

    #[test]
    fn shared_selection_preflight_skips_ineligible_objects_and_commits_only_on_success() {
        let mut document = Document::default();
        let ids = [0., 1., 2., 100., 200.].map(|x| {
            document
                .add_geometry(Geometry::Point(Point3::try_new(x, 0., 0.).unwrap()))
                .unwrap()
        });
        document.set_objects_visibility([ids[3]], false).unwrap();
        document.set_objects_locked([ids[4]], true).unwrap();
        let registry = CommandRegistry::with_builtins();
        registry.execute(&mut document, "Point 9,9,9").unwrap();
        registry.execute(&mut document, "Undo").unwrap();
        document
            .select_object(ids[0], SelectionMode::Replace)
            .unwrap();
        let original = document.objects().cloned().collect::<Vec<_>>();
        let undo = document.undo_label().map(str::to_owned);
        let redo = document.redo_label().map(str::to_owned);
        assert!(redo.is_some());
        let mut calls = 0;
        let result = select_matching_geometry(&mut document, |geometry| {
            let Geometry::Point(point) = geometry else {
                panic!("unexpected geometry")
            };
            assert!(point.x() < 100., "ineligible object reached predicate");
            calls += 1;
            if calls == 3 {
                Err(GeometryError::NumericalIntegrationDidNotConverge)
            } else {
                Ok(true)
            }
        });
        assert!(result.is_err());
        assert_eq!(calls, 3);
        assert_eq!(
            document.selected_object_ids().collect::<Vec<_>>(),
            vec![ids[0]]
        );
        assert_eq!(
            select_matching_geometry(&mut document, |_| Ok(false)).unwrap(),
            "Selected 1 object(s)"
        );
        assert_eq!(
            select_matching_geometry(&mut document, |_| Ok(true)).unwrap(),
            "Selected 3 object(s)"
        );
        assert_eq!(
            document.selected_object_ids().collect::<BTreeSet<_>>(),
            BTreeSet::from([ids[0], ids[1], ids[2]])
        );
        assert_eq!(document.objects().cloned().collect::<Vec<_>>(), original);
        assert_eq!(document.undo_label(), undo.as_deref());
        assert_eq!(document.redo_label(), redo.as_deref());
    }

    #[test]
    fn non_manifold_selection_uses_topology_and_preserves_document_history() {
        let tolerance = Tolerance::DEFAULT;
        let mesh = TriangleMesh::try_new(
            [
                [0., 0., 0.],
                [1., 0., 0.],
                [0., 1., 0.],
                [0., 0., 1.],
                [0., -1., 1.],
            ]
            .map(|p| Point3::try_from(p).unwrap())
            .to_vec(),
            vec![[0, 2, 1], [0, 1, 3], [0, 3, 2], [1, 2, 3], [0, 1, 4]],
            tolerance,
        )
        .unwrap();
        let brep = Brep::try_from_mesh(&mesh, true, tolerance).unwrap();
        assert!(!mesh.topology().is_manifold());
        assert!(!brep.is_manifold());
        let mut document = Document::default();
        let registry = CommandRegistry::with_builtins();
        let selected = document
            .add_geometry(Geometry::Point(Point3::try_new(9., 9., 9.).unwrap()))
            .unwrap();
        let mesh_id = document.add_geometry(Geometry::Mesh(mesh.clone())).unwrap();
        let brep_id = document.add_geometry(Geometry::Brep(brep)).unwrap();
        let open =
            TriangleMesh::try_new(mesh.vertices().to_vec(), vec![[0, 2, 1]], tolerance).unwrap();
        let face = Brep::try_from_mesh(&open, true, tolerance).unwrap();
        document.add_geometry(Geometry::Mesh(open)).unwrap();
        document.add_geometry(Geometry::Brep(face)).unwrap();
        let closed = TriangleMesh::try_new(
            mesh.vertices().to_vec(),
            vec![[0, 2, 1], [0, 1, 3], [0, 3, 2], [1, 2, 3]],
            tolerance,
        )
        .unwrap();
        document.add_geometry(Geometry::Mesh(closed)).unwrap();
        let hidden = document.add_geometry(Geometry::Mesh(mesh.clone())).unwrap();
        let locked = document.add_geometry(Geometry::Mesh(mesh)).unwrap();
        document.set_objects_visibility([hidden], false).unwrap();
        document.set_objects_locked([locked], true).unwrap();
        registry.execute(&mut document, "Point 8,8,8").unwrap();
        registry.execute(&mut document, "Undo").unwrap();
        document
            .select_object(selected, SelectionMode::Replace)
            .unwrap();
        let original = document.objects().cloned().collect::<Vec<_>>();
        let undo = document.undo_label().map(str::to_owned);
        let redo = document.redo_label().map(str::to_owned);
        assert_eq!(
            registry.execute(&mut document, "SelNonManifold").unwrap(),
            "Selected 3 object(s)"
        );
        assert_eq!(
            document.selected_object_ids().collect::<BTreeSet<_>>(),
            BTreeSet::from([selected, mesh_id, brep_id])
        );
        assert!(
            registry
                .execute(&mut document, "SelNonManifold extra")
                .is_err()
        );
        assert_eq!(document.objects().cloned().collect::<Vec<_>>(), original);
        assert_eq!(document.undo_label(), undo.as_deref());
        assert_eq!(document.redo_label(), redo.as_deref());
    }

    #[test]
    fn short_selection_integration_failure_preserves_selection_and_history() {
        let registry = CommandRegistry::with_builtins();
        let mut document = Document::default();
        registry.execute(&mut document, "Point 9,9").unwrap();
        let selected = document.objects().next().unwrap().id();
        registry.execute(&mut document, "Line 0,0 1,0").unwrap();
        // A valid curve whose tiny interior span cannot survive conversion
        // to a dimensionless parameter domain. A previously eligible line
        // must not become selected when the later curve fails preflight.
        let curve = NurbsCurve::try_new(
            1,
            (0..4)
                .map(|i| Point3::try_new(i as f64, 0., 0.).unwrap())
                .collect(),
            vec![
                -f64::MAX,
                -f64::MAX,
                0.,
                f64::from_bits(1),
                f64::MAX,
                f64::MAX,
            ],
        )
        .unwrap();
        document.add_geometry(Geometry::NurbsCurve(curve)).unwrap();
        document
            .select_object(selected, SelectionMode::Replace)
            .unwrap();
        let undo = document.undo_label().map(str::to_owned);
        let redo = document.redo_label().map(str::to_owned);
        assert!(registry.execute(&mut document, "SelShortCrv 10").is_err());
        assert_eq!(
            document.selected_object_ids().collect::<Vec<_>>(),
            vec![selected]
        );
        assert_eq!(document.objects().count(), 3);
        assert_eq!(document.undo_label(), undo.as_deref());
        assert_eq!(document.redo_label(), redo.as_deref());
    }

    #[test]
    fn nonlinear_short_selection_matches_recorded_rhino_representations() {
        let registry = CommandRegistry::with_builtins();
        let mut checked = 0;
        for input in [
            include_str!("../../../docs/short-curve-circle-measurement.json"),
            include_str!("../../../docs/short-curve-representation-measurement.json"),
        ] {
            let measurement: Value = serde_json::from_str(input).unwrap();
            for batch in measurement["batches"].as_array().unwrap() {
                assert_eq!(batch["response"]["engine"], "rhino");
                for operation in batch["request"]["operations"].as_array().unwrap() {
                    let mut document = Document::default();
                    let t = &batch["request"]["tolerance"];
                    document.set_tolerance(
                        Tolerance::try_new(
                            t["absolute"].as_f64().unwrap(),
                            t["relative"].as_f64().unwrap(),
                            t["angular"].as_f64().unwrap(),
                        )
                        .unwrap(),
                    );
                    for (i, length) in operation["lengths"].as_array().unwrap().iter().enumerate() {
                        let length = length.as_f64().unwrap();
                        let kind = operation["curve_kind"].as_str().unwrap();
                        let mut curve = if kind == "bezier_arch" {
                            let scale =
                                length / (0.5 * 5_f64.sqrt() + 0.25 * (2_f64 + 5_f64.sqrt()).ln());
                            NurbsCurve::try_new(
                                2,
                                vec![
                                    Point3::try_new(0., i as f64, 0.).unwrap(),
                                    Point3::try_new(0.5 * scale, i as f64 + scale, 0.).unwrap(),
                                    Point3::try_new(scale, i as f64, 0.).unwrap(),
                                ],
                                vec![0., 0., 0., 1., 1., 1.],
                            )
                            .unwrap()
                        } else {
                            assert!(matches!(kind, "circle" | "nurbs_circle"));
                            let circle = Circle3::try_new(
                                Point3::try_new(0., i as f64, 0.).unwrap(),
                                length / std::f64::consts::TAU,
                                Vector3::try_new(0., 0., 1.)
                                    .unwrap()
                                    .normalized_nonzero()
                                    .unwrap(),
                                document.tolerance(),
                            )
                            .unwrap();
                            if kind == "circle" {
                                document.add_geometry(Geometry::Circle(circle)).unwrap();
                                continue;
                            }
                            circle.to_nurbs().unwrap()
                        };
                        curve = curve
                            .try_change_degree(
                                operation["degree"].as_u64().unwrap_or(2) as usize,
                                false,
                            )
                            .unwrap();
                        for _ in 0..operation["refinement"].as_u64().unwrap_or(0) {
                            let midpoints = curve
                                .spans()
                                .map(|(a, b)| a + (b - a) * 0.5)
                                .collect::<Vec<_>>();
                            for midpoint in midpoints {
                                curve = curve.try_insert_knot(midpoint, 1).unwrap();
                            }
                        }
                        document.add_geometry(Geometry::NurbsCurve(curve)).unwrap();
                    }
                    let ids = document.objects().map(|o| o.id()).collect::<Vec<_>>();
                    registry
                        .execute(
                            &mut document,
                            &format!(
                                "SelShortCrv {}",
                                operation["maximum_length"].as_f64().unwrap()
                            ),
                        )
                        .unwrap();
                    let selected = document.selected_object_ids().collect::<BTreeSet<_>>();
                    let actual = ids
                        .iter()
                        .enumerate()
                        .filter_map(|(i, id)| selected.contains(id).then_some(i))
                        .collect::<Vec<_>>();
                    let result = batch["response"]["results"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .find(|r| r["id"] == operation["id"])
                        .unwrap();
                    let expected = result["value"]["selected"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|i| i.as_u64().unwrap() as usize)
                        .collect::<Vec<_>>();
                    assert_eq!(actual, expected, "{}", operation["id"]);
                    checked += ids.len();
                }
            }
        }
        assert_eq!(checked, 96);
    }

    #[test]
    fn analytic_circle_selection_matches_recorded_rhino_cases() {
        let measurement: Value = serde_json::from_str(include_str!(
            "../../../docs/short-curve-circle-measurement.json"
        ))
        .unwrap();
        let registry = CommandRegistry::with_builtins();
        let mut checked = 0;
        for batch in measurement["batches"].as_array().unwrap() {
            assert_eq!(batch["response"]["engine"], "rhino");
            // The companion rational cases are replayed by the broader
            // representation test above; keep this analytic regression focused.
            let operation = batch["request"]["operations"]
                .as_array()
                .unwrap()
                .iter()
                .find(|op| op["curve_kind"] == "circle")
                .unwrap();
            let expected = batch["response"]["results"]
                .as_array()
                .unwrap()
                .iter()
                .find(|r| r["id"] == operation["id"])
                .unwrap();
            let mut document = Document::default();
            let tolerance = &batch["request"]["tolerance"];
            document.set_tolerance(
                Tolerance::try_new(
                    tolerance["absolute"].as_f64().unwrap(),
                    tolerance["relative"].as_f64().unwrap(),
                    tolerance["angular"].as_f64().unwrap(),
                )
                .unwrap(),
            );
            for (i, length) in operation["lengths"].as_array().unwrap().iter().enumerate() {
                registry
                    .execute(
                        &mut document,
                        &format!(
                            "Circle 0,{i},0 {}",
                            length.as_f64().unwrap() / std::f64::consts::TAU
                        ),
                    )
                    .unwrap();
            }
            let ids = document.objects().map(|o| o.id()).collect::<Vec<_>>();
            registry
                .execute(
                    &mut document,
                    &format!(
                        "SelShortCrv {}",
                        operation["maximum_length"].as_f64().unwrap()
                    ),
                )
                .unwrap();
            let selected = document.selected_object_ids().collect::<BTreeSet<_>>();
            let actual = ids
                .iter()
                .enumerate()
                .filter_map(|(i, id)| selected.contains(id).then_some(i))
                .collect::<Vec<_>>();
            let expected = expected["value"]["selected"]
                .as_array()
                .unwrap()
                .iter()
                .map(|i| i.as_u64().unwrap() as usize)
                .collect::<Vec<_>>();
            assert_eq!(actual, expected);
            checked += ids.len();
        }
        assert_eq!(checked, 15);
    }

    #[test]
    fn short_curve_selection_matches_recorded_rhino_boundaries() {
        let measurement: Value = serde_json::from_str(include_str!(
            "../../../docs/short-curve-selection-measurement.json"
        ))
        .unwrap();
        let batches = measurement["batches"].as_array().unwrap();
        assert_eq!(batches.len(), 4);
        let registry = CommandRegistry::with_builtins();
        let mut checked = 0;
        for batch in batches {
            assert_eq!(batch["response"]["engine"], "rhino");
            let tolerance = &batch["request"]["tolerance"];
            for operation in batch["request"]["operations"].as_array().unwrap() {
                let mut document = Document::default();
                document.set_tolerance(
                    Tolerance::try_new(
                        tolerance["absolute"].as_f64().unwrap(),
                        tolerance["relative"].as_f64().unwrap(),
                        tolerance["angular"].as_f64().unwrap(),
                    )
                    .unwrap(),
                );
                for (i, length) in operation["lengths"].as_array().unwrap().iter().enumerate() {
                    registry
                        .execute(
                            &mut document,
                            &format!("Line 0,{i},0 {},{i},0", length.as_f64().unwrap()),
                        )
                        .unwrap();
                }
                let ids = document.objects().map(|o| o.id()).collect::<Vec<_>>();
                let history = document.undo_label().map(str::to_owned);
                registry
                    .execute(
                        &mut document,
                        &format!(
                            "SelShortCrv {}",
                            operation["maximum_length"].as_f64().unwrap()
                        ),
                    )
                    .unwrap();
                let selected = document.selected_object_ids().collect::<BTreeSet<_>>();
                let actual = ids
                    .iter()
                    .enumerate()
                    .filter_map(|(i, id)| selected.contains(id).then_some(i))
                    .collect::<Vec<_>>();
                let result = batch["response"]["results"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|r| r["id"] == operation["id"])
                    .unwrap();
                let expected = result["value"]["selected"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|i| i.as_u64().unwrap() as usize)
                    .collect::<Vec<_>>();
                assert_eq!(actual, expected, "{}", operation["id"]);
                assert_eq!(document.undo_label(), history.as_deref());
                checked += ids.len();
            }
        }
        assert_eq!(checked, 40);
    }
}
