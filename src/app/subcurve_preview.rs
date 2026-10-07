//! Read-only curve and endpoint previews share the command's geometry policy.
use super::*;
use std::sync::Arc;
use viboceros_command::subcurve_input;
use viboceros_document::GeometrySnapshot;
use viboceros_geometry::{GeometryError, NurbsCurve, Tolerance};

#[derive(Clone, Copy, Debug, PartialEq)]
struct Parameters {
    start: f64,
    hover: f64,
    length: Option<f64>,
    locked_forward: Option<bool>,
    midpoint: bool,
}

#[derive(Debug)]
pub(super) struct PreviewGeometry {
    pub(super) curve: NurbsCurve,
    pub(super) endpoints: [Point3; 2],
}

pub(super) struct Preview {
    pub(super) geometry: Arc<PreviewGeometry>,
    pub(super) show_curve: bool,
}

#[derive(Clone, Debug, Default)]
pub(super) struct Cache {
    key: Option<(GeometrySnapshot, Parameters, Tolerance)>,
    geometry: Option<Arc<PreviewGeometry>>,
    hover: Option<(GeometrySnapshot, Point3, Tolerance, Option<f64>)>,
}
impl Cache {
    pub(super) fn parameter(
        &mut self,
        source: Option<&GeometrySnapshot>,
        point: Point3,
        tolerance: Tolerance,
    ) -> Option<f64> {
        let Some(source) = source else {
            self.hover = None;
            return None;
        };
        if let Some((old_source, old_point, old_tolerance, parameter)) = &self.hover
            && old_source.shares_storage_with(source)
            && *old_point == point
            && *old_tolerance == tolerance
        {
            return *parameter;
        }
        let parameter = source
            .curve_ref()
            .and_then(|c| c.closest_parameter(point, tolerance).ok());
        self.hover = Some((source.clone(), point, tolerance, parameter));
        parameter
    }
    fn get(
        &mut self,
        source: Option<&GeometrySnapshot>,
        parameters: Option<Parameters>,
        tolerance: Tolerance,
    ) -> Option<Arc<PreviewGeometry>> {
        let Some((source, parameters)) = source.zip(parameters) else {
            self.key = None;
            self.geometry = None;
            return None;
        };
        if self
            .key
            .as_ref()
            .is_none_or(|(old_source, old_parameters, old_tolerance)| {
                !old_source.shares_storage_with(source)
                    || *old_parameters != parameters
                    || *old_tolerance != tolerance
            })
        {
            self.geometry = resolve(source, parameters, tolerance)
                .ok()
                .flatten()
                .map(Arc::new);
            self.key = Some((source.clone(), parameters, tolerance));
        }
        self.geometry.clone()
    }
}

fn resolve(
    source: &Geometry,
    p: Parameters,
    tolerance: Tolerance,
) -> Result<Option<PreviewGeometry>, GeometryError> {
    let Some(curve) = source.curve_ref() else {
        return Ok(None);
    };
    let piece = if p.midpoint {
        let radius = match p.length {
            Some(length) => length,
            None => subcurve_input::midpoint_radius(curve, p.start, p.hover, tolerance)?,
        };
        subcurve_input::midpoint_piece(curve, p.start, radius, tolerance)?
    } else if let Some(length) = p.length {
        if let Some(forward) = p.locked_forward {
            subcurve_input::locked_piece(curve, p.start, length, forward, tolerance)?
        } else {
            subcurve_input::piece(curve, p.start, p.hover, length, tolerance)?
        }
    } else {
        let Some([start, end]) =
            subcurve_input::point_parameters(curve, p.start, p.hover, p.locked_forward, true)?
        else {
            return Ok(None);
        };
        Some(curve.to_owned().try_subcurve(start, end)?)
    };
    let Some(piece) = piece else { return Ok(None) };
    let curve = piece.as_ref();
    Ok(Some(PreviewGeometry {
        endpoints: [curve.start_point()?, curve.end_point()?],
        curve: curve.to_nurbs()?,
    }))
}

impl VibocerosApp {
    pub(super) fn subcurve_preview_hover(&self) -> Option<f64> {
        self.subcurve_prompt
            .as_ref()
            .and_then(|p| p.hover_parameter)
            .or_else(|| {
                self.intersection_prompt
                    .as_ref()?
                    .uv_subcurves
                    .pending
                    .as_ref()?
                    .hover_parameter
            })
    }

    pub(super) fn subcurve_draft_preview(&mut self) -> Option<Preview> {
        let tolerance = self.document.tolerance();
        if let Some(p) = self.subcurve_prompt.as_mut() {
            let source = p
                .source
                .filter(|id| self.document.is_object_selectable(*id))
                .and_then(|id| self.document.object(id))
                .map(|o| o.geometry_snapshot());
            let parameters = p
                .start
                .zip(p.hover_parameter)
                .map(|(start, hover)| Parameters {
                    start,
                    hover,
                    length: p.length,
                    locked_forward: p.locked_forward,
                    midpoint: p.from_midpoint,
                });
            return p
                .preview
                .get(source, parameters, tolerance)
                .map(|geometry| Preview {
                    geometry,
                    show_curve: p.mode == subcurve_input::SubcurveMode::Shorten,
                });
        }
        let p = self
            .intersection_prompt
            .as_mut()?
            .uv_subcurves
            .pending
            .as_mut()?;
        let source = p
            .object
            .filter(|id| self.document.is_object_selectable(*id))
            .and_then(|id| self.document.object(id))
            .map(|o| o.geometry_snapshot());
        let parameters = p
            .start
            .zip(p.hover_parameter)
            .map(|(start, hover)| Parameters {
                start,
                hover,
                length: p.length,
                locked_forward: p.locked_forward,
                midpoint: false,
            });
        p.preview
            .get(source, parameters, tolerance)
            .map(|geometry| Preview {
                geometry,
                show_curve: true,
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use viboceros_geometry::LineSegment;
    fn source(end: [f64; 3]) -> GeometrySnapshot {
        Geometry::Line(
            LineSegment::try_new(
                Point3::try_new(0., 0., 0.).unwrap(),
                Point3::try_from(end).unwrap(),
                Tolerance::DEFAULT,
            )
            .unwrap(),
        )
        .into()
    }
    #[test]
    fn cache_invalidates_geometry_hover_and_failed_results_by_snapshot_and_inputs() {
        let x = source([10., 0., 0.]);
        let y = source([0., 10., 0.]);
        let mut cache = Cache::default();
        let p = Parameters {
            start: 2.,
            hover: 4.,
            length: None,
            locked_forward: None,
            midpoint: false,
        };
        let first = cache.get(Some(&x), Some(p), Tolerance::DEFAULT).unwrap();
        assert!(Arc::ptr_eq(
            &first,
            &cache.get(Some(&x), Some(p), Tolerance::DEFAULT).unwrap()
        ));
        let new = cache.get(Some(&y), Some(p), Tolerance::DEFAULT).unwrap();
        assert!(!Arc::ptr_eq(&first, &new));
        assert_eq!(new.endpoints[0], Point3::try_new(0., 2., 0.).unwrap());
        let point = Point3::try_new(6., 4., 0.).unwrap();
        assert_eq!(
            cache.parameter(Some(&x), point, Tolerance::DEFAULT),
            Some(6.)
        );
        assert_eq!(
            cache.parameter(Some(&y), point, Tolerance::DEFAULT),
            Some(4.)
        );
        let bad = Parameters { hover: 2., ..p };
        assert!(cache.get(Some(&x), Some(bad), Tolerance::DEFAULT).is_none());
        assert!(cache.get(Some(&x), Some(bad), Tolerance::DEFAULT).is_none());
        assert!(cache.key.as_ref().unwrap().1 == bad);
        assert!(cache.get(None, Some(p), Tolerance::DEFAULT).is_none());
        assert!(cache.key.is_none());
        assert!(cache.get(Some(&x), Some(p), Tolerance::DEFAULT).is_some());
        assert!(cache.parameter(None, point, Tolerance::DEFAULT).is_none());
        assert!(cache.hover.is_none());
    }
}
