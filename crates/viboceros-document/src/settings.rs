//! Undoable document settings which do not transform existing geometry.

use super::{Document, Edit};
use viboceros_geometry::Tolerance;

impl Document {
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
