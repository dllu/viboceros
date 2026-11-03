//! Model policy and analysis display settings.

use super::{Document, Edit};
use viboceros_geometry::Tolerance;

impl Document {
    pub fn analysis_mesh_parameters(&self) -> Option<&std::sync::Arc<[u8]>> {
        self.analysis_mesh_parameters.as_ref()
    }
    pub fn set_analysis_mesh_parameters(&mut self, parameters: Option<std::sync::Arc<[u8]>>) {
        self.analysis_mesh_parameters = parameters;
    }
    pub const fn analysis_simple_planes(&self) -> bool {
        self.analysis_simple_planes
    }
    pub fn set_analysis_simple_planes(&mut self, simple: bool) {
        self.analysis_simple_planes = simple;
    }
    pub fn restore_analysis_mesh_settings(&mut self) {
        self.analysis_mesh_parameters = None;
        self.analysis_simple_planes = false;
        self.analysis_nurbs_density = Default::default();
        self.analysis_subd_density = Default::default();
    }
    pub const fn analysis_nurbs_density(&self) -> viboceros_geometry::NurbsMeshDensity {
        self.analysis_nurbs_density
    }
    pub fn set_analysis_nurbs_density(
        &mut self,
        density: viboceros_geometry::NurbsMeshDensity,
    ) -> bool {
        if self.analysis_nurbs_density == density {
            return false;
        }
        self.analysis_nurbs_density = density;
        true
    }
    pub const fn analysis_mesh_settings(&self) -> viboceros_geometry::AnalysisMeshSettings {
        viboceros_geometry::AnalysisMeshSettings {
            simple_planes: self.analysis_simple_planes,
            nurbs_density: self.analysis_nurbs_density,
            subd_density: self.analysis_subd_density,
        }
    }
    pub const fn analysis_subd_density(&self) -> viboceros_geometry::SubDMeshDensity {
        self.analysis_subd_density
    }

    /// Analysis display settings are saved with the document but do not edit
    /// control geometry, selection, or model undo/redo history.
    pub fn set_analysis_subd_density(
        &mut self,
        density: viboceros_geometry::SubDMeshDensity,
    ) -> bool {
        if self.analysis_subd_density == density {
            return false;
        }
        self.analysis_subd_density = density;
        true
    }

    /// Sets validated model tolerances for subsequent operations.
    /// Existing geometry, attributes, selection, and units are untouched; this
    /// does not refit geometry or reinterpret stored B-rep edge tolerances.
    /// Returns false for a no-op, preserving redo. Changes join an active
    /// transaction or create one standalone undo step.
    pub fn set_tolerance(&mut self, tolerance: Tolerance) -> bool {
        if self.tolerance == tolerance {
            return false;
        }
        let previous = std::mem::replace(&mut self.tolerance, tolerance);
        self.record_edit(
            "Tolerance",
            Edit::ToleranceChanged {
                tolerance: previous,
            },
        );
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Geometry;
    use viboceros_geometry::{LengthUnitSystem, LineSegment, Point3};

    #[test]
    fn analysis_mesh_settings_survive_block_edit_save_cancel_and_history() {
        use viboceros_geometry::{NurbsMeshDensity, SubDMeshDensity};
        for save in [false, true] {
            let mut doc = Document::default();
            let point = Point3::try_new(1., 2., 3.).unwrap();
            let id = doc.add_geometry(Geometry::Point(point)).unwrap();
            let (_, instance) = doc.create_block_from_objects("part", point, [id]).unwrap();
            doc.open_block_edit(instance).unwrap();
            let density = SubDMeshDensity::new(2, false).unwrap();
            doc.set_analysis_subd_density(density);
            let nurbs = NurbsMeshDensity::new(0.25).unwrap();
            doc.set_analysis_nurbs_density(nurbs);
            doc.set_analysis_simple_planes(true);
            let parameters: std::sync::Arc<[u8]> = std::sync::Arc::from([17, 23, 41]);
            doc.set_analysis_mesh_parameters(Some(parameters.clone()));
            if save {
                doc.save_block_edit().unwrap();
            } else {
                doc.discard_block_edit().unwrap();
            }
            assert_eq!(doc.analysis_subd_density(), density);
            assert_eq!(doc.analysis_nurbs_density(), nurbs);
            assert!(doc.analysis_simple_planes());
            assert_eq!(doc.analysis_mesh_parameters(), Some(&parameters));
            doc.undo().unwrap();
            assert_eq!(doc.analysis_subd_density(), density);
            assert_eq!(doc.analysis_nurbs_density(), nurbs);
            doc.redo().unwrap();
            assert_eq!(doc.analysis_subd_density(), density);
            assert_eq!(doc.analysis_nurbs_density(), nurbs);
            assert!(doc.analysis_simple_planes());
            assert_eq!(doc.analysis_mesh_parameters(), Some(&parameters));
        }
    }

    #[test]
    fn tolerance_edits_preserve_existing_geometry_units_and_selection() {
        let mut document = Document::new(Tolerance::DEFAULT);
        let line = LineSegment::try_new(
            Point3::try_new(0.0, 0.0, 0.0).unwrap(),
            Point3::try_new(1e-5, 0.0, 0.0).unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        let id = document.add_geometry(Geometry::Line(line)).unwrap();
        document
            .select_object(id, crate::SelectionMode::Replace)
            .unwrap();
        let objects = document.objects().cloned().collect::<Vec<_>>();
        let selected = document.selected_object_ids().collect::<Vec<_>>();
        let tolerance = Tolerance::try_new(0.01, 1e-6, 0.001).unwrap();
        assert!(document.set_tolerance(tolerance));
        assert_eq!(document.tolerance(), tolerance);
        assert_eq!(document.objects().cloned().collect::<Vec<_>>(), objects);
        assert_eq!(document.selected_object_ids().collect::<Vec<_>>(), selected);
        assert_eq!(document.units(), &LengthUnitSystem::Millimeters);
        for _ in 0..16 {
            document.undo().unwrap();
            assert_eq!(document.tolerance(), Tolerance::DEFAULT);
            assert_eq!(document.objects().cloned().collect::<Vec<_>>(), objects);
            let before = format!("{document:?}");
            assert!(!document.set_tolerance(Tolerance::DEFAULT));
            assert_eq!(format!("{document:?}"), before);
            document.redo().unwrap();
            assert_eq!(document.tolerance(), tolerance);
        }
    }

    #[test]
    fn tolerance_settings_do_not_replace_last_changed_object_tracking() {
        let mut document = Document::new(Tolerance::DEFAULT);
        let id = document
            .add_geometry(Geometry::Point(Point3::try_new(1.0, 0.0, 0.0).unwrap()))
            .unwrap();
        document.set_tolerance(Tolerance::try_new(0.01, 1e-6, 0.001).unwrap());
        assert_eq!(document.select_last_changed(true), 1);
        assert_eq!(document.selected_object_ids().collect::<Vec<_>>(), [id]);
        document.undo().unwrap();
        assert_eq!(document.select_last_changed(true), 1);
        assert_eq!(document.selected_object_ids().collect::<Vec<_>>(), [id]);
        document.redo().unwrap();
        assert_eq!(document.select_last_changed(true), 1);
        assert_eq!(document.selected_object_ids().collect::<Vec<_>>(), [id]);
    }

    #[test]
    fn tolerance_and_unit_edits_rollback_and_replay_in_transaction_order() {
        let mut document = Document::new(Tolerance::DEFAULT);
        document
            .add_geometry(Geometry::Point(Point3::try_new(1000.0, 0.0, 0.0).unwrap()))
            .unwrap();
        let objects = document.objects().cloned().collect::<Vec<_>>();
        let first = Tolerance::try_new(0.001, 1e-6, 0.01).unwrap();
        let second = Tolerance::try_new(0.1, 1e-4, 0.1).unwrap();
        for commit in [false, true] {
            document.begin_transaction("Model settings").unwrap();
            document.set_tolerance(first);
            document.set_units(LengthUnitSystem::Meters, true).unwrap();
            document.set_tolerance(second);
            if commit {
                document.commit_transaction().unwrap();
                document.undo().unwrap();
            } else {
                document.rollback_transaction().unwrap();
            }
            assert_eq!(document.tolerance(), Tolerance::DEFAULT);
            assert_eq!(document.units(), &LengthUnitSystem::Millimeters);
            assert_eq!(document.objects().cloned().collect::<Vec<_>>(), objects);
            if commit {
                document.redo().unwrap();
                assert_eq!(document.tolerance(), second);
                assert_eq!(document.units(), &LengthUnitSystem::Meters);
                assert_eq!(
                    document.objects().next().unwrap().geometry(),
                    &Geometry::Point(Point3::try_new(1.0, 0.0, 0.0).unwrap())
                );
            }
        }
    }
}
