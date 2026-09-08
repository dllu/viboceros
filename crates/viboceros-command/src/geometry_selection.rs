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
        let matches = document
            .objects()
            .filter(|object| document.is_object_selectable(object.id()))
            .map(|object| {
                Ok(self
                    .filter
                    .matches(object.geometry(), tolerance)?
                    .then_some(object.id()))
            })
            .collect::<Result<Vec<Option<ObjectId>>, GeometryError>>()?
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
        document.select_objects(matches, SelectionMode::Add)?;
        Ok(format!(
            "Selected {} object(s)",
            document.selected_object_count()
        ))
    }
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
        let tolerance = document.tolerance();
        let matches = document
            .objects()
            .filter(|object| document.is_object_selectable(object.id()))
            .filter_map(|object| {
                geometry_curve_ref(object.geometry()).map(|curve| (object.id(), curve))
            })
            .map(|(id, curve)| Ok((curve.length(tolerance)? <= comparison_limit).then_some(id)))
            .collect::<Result<Vec<Option<ObjectId>>, GeometryError>>()?
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
        document.select_objects(matches, SelectionMode::Add)?;
        Ok(format!(
            "Selected {} object(s)",
            document.selected_object_count()
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

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
