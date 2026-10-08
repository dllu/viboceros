//! Read-only curve and endpoint previews share the command's geometry policy.
use super::*;
use std::sync::Arc;
use viboceros_command::subcurve_input;
use viboceros_document::GeometrySnapshot;
use viboceros_geometry::{CurveRef, GeometryError, NurbsCurve, Tolerance};

#[derive(Clone, Copy, Debug, PartialEq)]
struct Parameters {
    edge: Option<usize>,
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

#[derive(Clone, Debug)]
struct HoverLookup {
    source: GeometrySnapshot,
    edge: Option<usize>,
    point: Point3,
    tolerance: Tolerance,
    parameter: Option<f64>,
}

#[derive(Clone, Debug, Default)]
pub(super) struct Cache {
    surface_edge: Option<(GeometrySnapshot, usize, Tolerance, Option<NurbsCurve>)>,
    key: Option<(GeometrySnapshot, Parameters, Tolerance)>,
    geometry: Option<Arc<PreviewGeometry>>,
    hover: Option<HoverLookup>,
}
impl Cache {
    fn resolved<'a>(
        &'a mut self,
        source: &'a GeometrySnapshot,
        edge: Option<usize>,
        tolerance: Tolerance,
    ) -> Option<CurveRef<'a>> {
        if let (Geometry::NurbsSurface(_), Some(index)) = (&**source, edge) {
            if self
                .surface_edge
                .as_ref()
                .is_none_or(|(old, old_index, old_tolerance, _)| {
                    !old.shares_storage_with(source)
                        || *old_index != index
                        || *old_tolerance != tolerance
                })
            {
                let curve = viboceros_command::curve_reference::resolve(source, edge, tolerance)
                    .and_then(|c| c.curve().to_nurbs().ok());
                self.surface_edge = Some((source.clone(), index, tolerance, curve));
            }
            return self
                .surface_edge
                .as_ref()?
                .3
                .as_ref()
                .map(CurveRef::NurbsCurve);
        }
        self.surface_edge = None;
        match edge {
            None => source.curve_ref(),
            Some(index) => match &**source {
                Geometry::Brep(brep) => brep
                    .edges()
                    .get(index)
                    .map(|e| CurveRef::NurbsCurve(e.curve())),
                _ => None,
            },
        }
    }

    pub(super) fn parameter(
        &mut self,
        source: Option<&GeometrySnapshot>,
        edge: Option<usize>,
        point: Point3,
        tolerance: Tolerance,
    ) -> Option<f64> {
        let Some(source) = source else {
            self.hover = None;
            return None;
        };
        if let Some(old) = &self.hover
            && old.source.shares_storage_with(source)
            && old.point == point
            && old.edge == edge
            && old.tolerance == tolerance
        {
            return old.parameter;
        }
        let parameter = self
            .resolved(source, edge, tolerance)
            .and_then(|c| c.closest_parameter(point, tolerance).ok());
        self.hover = Some(HoverLookup {
            source: source.clone(),
            edge,
            point,
            tolerance,
            parameter,
        });
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
            self.geometry = self
                .resolved(source, parameters.edge, tolerance)
                .and_then(|curve| resolve(curve, parameters, tolerance).ok().flatten())
                .map(Arc::new);
            self.key = Some((source.clone(), parameters, tolerance));
        }
        self.geometry.clone()
    }
}

fn resolve(
    curve: CurveRef<'_>,
    p: Parameters,
    tolerance: Tolerance,
) -> Result<Option<PreviewGeometry>, GeometryError> {
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
        if !self.validate_subcurve_source() {
            return None;
        }
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
                    edge: p.edge,
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
                edge: None,
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
            edge: None,
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
            cache.parameter(Some(&x), None, point, Tolerance::DEFAULT),
            Some(6.)
        );
        assert_eq!(
            cache.parameter(Some(&y), None, point, Tolerance::DEFAULT),
            Some(4.)
        );
        let bad = Parameters { hover: 2., ..p };
        assert!(cache.get(Some(&x), Some(bad), Tolerance::DEFAULT).is_none());
        assert!(cache.get(Some(&x), Some(bad), Tolerance::DEFAULT).is_none());
        assert!(cache.key.as_ref().unwrap().1 == bad);
        assert!(cache.get(None, Some(p), Tolerance::DEFAULT).is_none());
        assert!(cache.key.is_none());
        assert!(cache.get(Some(&x), Some(p), Tolerance::DEFAULT).is_some());
        assert!(
            cache
                .parameter(None, None, point, Tolerance::DEFAULT)
                .is_none()
        );
        assert!(cache.hover.is_none());
    }
    #[test]
    fn surface_edge_conversion_is_reused_across_hover_and_invalidated_by_edge_or_snapshot() {
        let surface = viboceros_geometry::NurbsSurface::try_bilinear([
            Point3::try_new(0., 0., 0.).unwrap(),
            Point3::try_new(4., 0., 0.).unwrap(),
            Point3::try_new(4., 6., 0.).unwrap(),
            Point3::try_new(0., 6., 0.).unwrap(),
        ])
        .unwrap();
        let source: GeometrySnapshot = Geometry::NurbsSurface(surface).into();
        let mut cache = Cache::default();
        let a = Point3::try_new(1., 0., 0.).unwrap();
        let b = Point3::try_new(3., 0., 0.).unwrap();
        let start = cache
            .parameter(Some(&source), Some(0), a, Tolerance::DEFAULT)
            .unwrap();
        let storage = cache
            .surface_edge
            .as_ref()
            .unwrap()
            .3
            .as_ref()
            .unwrap()
            .control_points()
            .as_ptr();
        let hover = cache
            .parameter(Some(&source), Some(0), b, Tolerance::DEFAULT)
            .unwrap();
        assert_eq!(
            storage,
            cache
                .surface_edge
                .as_ref()
                .unwrap()
                .3
                .as_ref()
                .unwrap()
                .control_points()
                .as_ptr()
        );
        let p = Parameters {
            edge: Some(0),
            start,
            hover,
            length: None,
            locked_forward: None,
            midpoint: false,
        };
        let preview = cache
            .get(Some(&source), Some(p), Tolerance::DEFAULT)
            .unwrap();
        assert!(preview.endpoints[0].distance_to(a).unwrap() < 1e-9);
        assert!(preview.endpoints[1].distance_to(b).unwrap() < 1e-9);
        assert_eq!(
            storage,
            cache
                .surface_edge
                .as_ref()
                .unwrap()
                .3
                .as_ref()
                .unwrap()
                .control_points()
                .as_ptr()
        );
        assert!(
            cache
                .parameter(Some(&source), Some(999), b, Tolerance::DEFAULT)
                .is_none()
        );
        assert!(cache.surface_edge.as_ref().unwrap().3.is_none());
        assert!(
            cache
                .parameter(Some(&source), Some(0), b, Tolerance::DEFAULT)
                .is_some()
        );
        let other: GeometrySnapshot = Geometry::NurbsSurface(
            viboceros_geometry::NurbsSurface::try_bilinear([
                Point3::try_new(0., 0., 2.).unwrap(),
                Point3::try_new(4., 0., 2.).unwrap(),
                Point3::try_new(4., 6., 2.).unwrap(),
                Point3::try_new(0., 6., 2.).unwrap(),
            ])
            .unwrap(),
        )
        .into();
        assert!(
            cache
                .parameter(Some(&other), Some(0), b, Tolerance::DEFAULT)
                .is_some()
        );
        assert!(
            cache
                .surface_edge
                .as_ref()
                .unwrap()
                .0
                .shares_storage_with(&other)
        );
        let new = cache
            .get(Some(&other), Some(p), Tolerance::DEFAULT)
            .unwrap();
        assert_eq!(new.endpoints[0].z(), 2.);
        let tolerance = Tolerance::try_new(1e-4, 1e-12, 1e-10).unwrap();
        assert!(
            cache
                .parameter(Some(&other), Some(0), b, tolerance)
                .is_some()
        );
        assert_eq!(cache.surface_edge.as_ref().unwrap().2, tolerance);
    }
}
