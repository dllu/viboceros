//! Cache model-space preview geometry independently of viewport navigation.

use std::sync::Arc;
use viboceros_geometry::{ControlPointCurveClosure, NurbsCurve, Point3};

#[derive(Default)]
pub(super) struct CurvePreviewCache {
    key: Option<(usize, ControlPointCurveClosure, Vec<Point3>)>,
    curve: Option<Arc<NurbsCurve>>,
}

impl CurvePreviewCache {
    pub(super) fn get(
        &mut self,
        settings: Option<(usize, ControlPointCurveClosure)>,
        points: &[Point3],
    ) -> Option<Arc<NurbsCurve>> {
        let Some((degree, closure)) = settings else {
            self.key = None;
            self.curve = None;
            return None;
        };
        if self
            .key
            .as_ref()
            .is_none_or(|(cached_degree, cached_closure, cached_points)| {
                *cached_degree != degree || *cached_closure != closure || cached_points != points
            })
        {
            self.curve =
                NurbsCurve::try_control_point_curve_with_closure(degree, points.to_vec(), closure)
                    .ok()
                    .map(Arc::new);
            // Failed constructions are cached too, until their inputs change.
            self.key = Some((degree, closure, points.to_vec()));
        }
        self.curve.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unchanged_inputs_reuse_geometry_and_every_input_change_invalidates_it() {
        let mut cache = CurvePreviewCache::default();
        let mut points = vec![
            Point3::try_new(0., 0., 0.).unwrap(),
            Point3::try_new(1., 0., 0.).unwrap(),
            Point3::try_new(0., 1., 0.).unwrap(),
        ];
        let open = Some((2, ControlPointCurveClosure::Open));
        let first = cache.get(open, &points).unwrap();
        for _ in 0..100 {
            assert!(Arc::ptr_eq(&first, &cache.get(open, &points).unwrap()));
        }
        points[1] = Point3::try_new(2., 0., 0.).unwrap();
        let moved = cache.get(open, &points).unwrap();
        assert!(!Arc::ptr_eq(&first, &moved));
        let degree = cache
            .get(Some((1, ControlPointCurveClosure::Open)), &points)
            .unwrap();
        assert!(!Arc::ptr_eq(&moved, &degree));
        let closed = cache
            .get(Some((1, ControlPointCurveClosure::Sharp)), &points)
            .unwrap();
        assert!(!Arc::ptr_eq(&degree, &closed));
        assert!(closed.is_closed().unwrap());
        assert!(cache.get(None, &points).is_none());
        assert!(cache.key.is_none());
        assert!(cache.curve.is_none());
        assert!(!Arc::ptr_eq(&first, &cache.get(open, &points).unwrap()));
    }

    #[test]
    fn invalid_drafts_replace_cached_geometry_and_recover_when_corrected() {
        let mut cache = CurvePreviewCache::default();
        let settings = Some((3, ControlPointCurveClosure::Open));
        let points = [
            Point3::try_new(0., 0., 0.).unwrap(),
            Point3::try_new(1., 0., 0.).unwrap(),
        ];
        assert!(cache.get(settings, &points).is_some());
        assert!(cache.get(settings, &points[..1]).is_none());
        assert_eq!(cache.key.as_ref().unwrap().2, points[..1]);
        assert!(cache.get(settings, &points[..1]).is_none());
        assert!(cache.get(settings, &points).is_some());
    }
}
