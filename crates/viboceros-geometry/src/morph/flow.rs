//! Curve-to-curve arc-length coordinates with shared prepared guide data.
use super::*;
use crate::{Curve3, CurveRef, FrameTransportOptions, ParameterSide, curve::ArcLengthSampler};
use std::sync::Arc;
mod local;
pub use local::CurveFlowLocalRadii;

/// Immutable underlying surface and B-rep face orientation for target sections.
#[derive(Clone, Debug)]
pub struct CurveFlowTargetSurface {
    surface: Arc<NurbsSurface>,
    reversed: bool,
}
impl CurveFlowTargetSurface {
    pub fn new(surface: Arc<NurbsSurface>, reversed: bool) -> Self {
        Self { surface, reversed }
    }
    fn normal(&self, point: Point3, tolerance: Tolerance) -> Result<UnitVector3, GeometryError> {
        let (u, v) = self.surface.closest_parameters(point, tolerance)?;
        let normal = self.surface.normal_at(u, v)?;
        Ok(if self.reversed {
            normal.opposite()
        } else {
            normal
        })
    }
}

const FLOW_LOOKUP_INTERVALS: usize = 840;
const FLOW_INVERSE_TOLERANCE: Real = 1e-6;
const FLOW_LENGTH_TOLERANCE: Real = 1e-8;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CurveFlowOptions {
    pub stretch: bool,
    pub reverse_base: bool,
    pub reverse_target: bool,
    pub preserve_structure: bool,
    pub base_up: Option<Vector3>,
    pub target_up: Option<Vector3>,
    pub local_radii: Option<CurveFlowLocalRadii>,
}
impl Default for CurveFlowOptions {
    fn default() -> Self {
        Self {
            stretch: true,
            reverse_base: false,
            reverse_target: false,
            preserve_structure: false,
            base_up: None,
            target_up: None,
            local_radii: None,
        }
    }
}

/// A Flow section can lose both transverse directions at a Roadlike pole.
/// Keep that rank-one map separate from Frame3's orthonormal invariant.
#[derive(Clone, Copy, Debug)]
struct Section {
    origin: Point3,
    axes: [Vector3; 3],
}
impl Section {
    fn from_frame(frame: Frame3) -> Self {
        Self {
            origin: frame.origin(),
            axes: frame.axes().map(UnitVector3::as_vector),
        }
    }
    fn axial(origin: Point3, tangent: UnitVector3) -> Result<Self, GeometryError> {
        let zero = Vector3::try_new(0., 0., 0.)?;
        Ok(Self {
            origin,
            axes: [tangent.as_vector(), zero, zero],
        })
    }
    fn with_origin(self, origin: Point3) -> Self {
        Self { origin, ..self }
    }
}

#[derive(Debug)]
struct Guide {
    length: ArcLengthSampler<'static>,
    lookup: Option<NurbsLookup>,
    normal: Option<UnitVector3>,
    surface: Option<CurveFlowTargetSurface>,
    parameters: Vec<Real>,
    frames: Vec<Frame3>,
    tolerance: Tolerance,
}

// Build the native Flow lookup policy from independent quadrature. The default
// guide length and the sequential inverse use separate numerical tolerances.
#[derive(Debug)]
struct NurbsLookup {
    parameters: Vec<Real>,
    length: Real,
    mapped_length: Real,
}
impl NurbsLookup {
    fn needed(curve: CurveRef<'_>) -> Result<bool, GeometryError> {
        match curve {
            CurveRef::NurbsCurve(c) => Ok(!c.is_linear_at_zero_tolerance()?),
            CurveRef::PolyCurve(c) => {
                for segment in c.segments() {
                    if Self::needed(segment.as_ref())? {
                        return Ok(true);
                    }
                }
                Ok(false)
            }
            _ => Ok(false),
        }
    }
    fn new(sampler: &ArcLengthSampler<'_>) -> Result<Self, GeometryError> {
        let fractions = (0..=FLOW_LOOKUP_INTERVALS)
            .map(|i| i as Real / FLOW_LOOKUP_INTERVALS as Real)
            .collect::<Vec<_>>();
        let parameters = sampler
            .parameters_at_incremental_normalized_lengths(&fractions, FLOW_INVERSE_TOLERANCE)?;
        let length = sampler
            .integration_checkpoints(FLOW_LENGTH_TOLERANCE)?
            .last()
            .unwrap()
            .distance();
        if !length.is_finite() || length <= 0. {
            return Err(GeometryError::NumericalIntegrationDidNotConverge);
        }
        Ok(Self {
            parameters,
            length,
            mapped_length: length,
        })
    }
    fn prefix(
        &self,
        sampler: &ArcLengthSampler<'_>,
        length: Real,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        // Unstretched Flow uses the source-length prefix of a longer target.
        // Its cut and the prefix's inverse table each use incremental queries;
        // scaling a full-curve table gives different quadrature checkpoints.
        let fraction = crate::parameter::scaled_ratio(1., length, self.length)?;
        let cut = sampler
            .parameters_at_incremental_normalized_lengths(&[fraction], FLOW_INVERSE_TOLERANCE)?[0];
        let curve = sampler
            .source_curve()
            .to_owned()
            .try_trimmed(*sampler.source_curve().domain().start()..=cut)?;
        let prefix = ArcLengthSampler::try_new(curve.as_ref(), tolerance)?;
        let fractions = (0..=FLOW_LOOKUP_INTERVALS)
            .map(|i| i as Real / FLOW_LOOKUP_INTERVALS as Real)
            .collect::<Vec<_>>();
        Ok(Self {
            parameters: prefix
                .parameters_at_incremental_normalized_lengths(&fractions, FLOW_INVERSE_TOLERANCE)?,
            length: self.length,
            mapped_length: length,
        })
    }
    fn parameter(&self, fraction: Real) -> Real {
        let u = fraction.clamp(0., 1.) * FLOW_LOOKUP_INTERVALS as Real;
        let i = (u.floor() as usize).min(FLOW_LOOKUP_INTERVALS - 1);
        let f = u - i as Real;
        self.parameters[i] * (1. - f) + self.parameters[i + 1] * f
    }
    fn fraction(&self, parameter: Real) -> Real {
        if parameter <= self.parameters[0] {
            return 0.;
        }
        if parameter >= self.parameters[FLOW_LOOKUP_INTERVALS] {
            return 1.;
        }
        let i = self
            .parameters
            .partition_point(|t| *t <= parameter)
            .saturating_sub(1)
            .min(FLOW_LOOKUP_INTERVALS - 1);
        let f = (parameter - self.parameters[i]) / (self.parameters[i + 1] - self.parameters[i]);
        (i as Real + f) / FLOW_LOOKUP_INTERVALS as Real
    }
}
impl Guide {
    fn new(
        curve: Arc<Curve3>,
        reverse: bool,
        up: Option<Vector3>,
        surface: Option<CurveFlowTargetSurface>,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        let curve = if reverse {
            Arc::new(curve.reversed(tolerance)?)
        } else {
            curve
        };
        let reference = curve.as_ref().as_ref();
        let normal = match up {
            Some(up) => Some(up.normalized_nonzero()?),
            None => reference.supporting_plane_normal(tolerance)?,
        };
        let mut parameters = Vec::new();
        let mut frames = Vec::new();
        if normal.is_none() && surface.is_none() {
            for i in 0..=64 {
                let t = reference.parameter_at(i as Real / 64.)?;
                if parameters.last().is_none_or(|last| *last < t) {
                    parameters.push(t);
                }
            }
            let start = reference.evaluate_with_tangent(parameters[0])?;
            let vertical = Vector3::try_new(0., 0., 1.)?;
            let side = vertical.cross(start.tangent().as_vector())?;
            let seed = if side.length()? > 64. * Real::EPSILON {
                side.normalized_nonzero()?
            } else {
                Frame3::try_from_normal(
                    start.point(),
                    start.tangent().as_vector(),
                    Tolerance::NUMERICAL_VALIDATION,
                )?
                .x_axis()
            };
            frames = reference.rotation_minimizing_frames(
                &parameters,
                Some(seed),
                FrameTransportOptions::default(),
            )?;
        }
        let mut length = ArcLengthSampler::try_new_owned(curve, tolerance)?;
        length.prepare_budgeted_repeated_sampling(64)?;
        let lookup = if NurbsLookup::needed(length.source_curve())? {
            Some(NurbsLookup::new(&length)?)
        } else {
            None
        };
        Ok(Self {
            length,
            lookup,
            normal,
            surface,
            parameters,
            frames,
            tolerance,
        })
    }
    fn curve(&self) -> CurveRef<'_> {
        self.length.source_curve()
    }
    fn frame(&self, t: Real) -> Result<Section, GeometryError> {
        self.frame_at_point(t, None)
    }
    fn frame_at_point(
        &self,
        t: Real,
        normal_point: Option<Point3>,
    ) -> Result<Section, GeometryError> {
        let curve = self.curve();
        let side = if t == *curve.domain().end() {
            ParameterSide::Left
        } else {
            ParameterSide::Right
        };
        let sample = curve.evaluate_with_tangent_on_side(t, side)?;
        let normal = if let Some(surface) = &self.surface {
            Some(surface.normal(normal_point.unwrap_or(sample.point()), self.tolerance)?)
        } else {
            self.normal
        };
        let y = if let Some(n) = normal {
            let cross = n.as_vector().cross(sample.tangent().as_vector())?;
            if cross.length()? == 0. {
                return Section::axial(sample.point(), sample.tangent());
            }
            cross.normalized_nonzero()?
        } else {
            let index = self
                .parameters
                .partition_point(|p| *p <= t)
                .saturating_sub(1);
            let frame = self.frames[index];
            if self.parameters[index] == t {
                frame.x_axis()
            } else {
                curve
                    .rotation_minimizing_frames(
                        &[self.parameters[index], t],
                        Some(frame.x_axis()),
                        FrameTransportOptions {
                            angular_tolerance: 1e-12,
                            side,
                            ..Default::default()
                        },
                    )?
                    .pop()
                    .unwrap()
                    .x_axis()
            }
        };
        Ok(Section::from_frame(Frame3::try_from_directions(
            sample.point(),
            sample.tangent().as_vector(),
            y.as_vector(),
            Tolerance::NUMERICAL_VALIDATION,
        )?))
    }
    fn at_distance(&self, distance: Real, along: Real) -> Result<Section, GeometryError> {
        let length = self
            .lookup
            .as_ref()
            .map_or(self.total_length(), |q| q.mapped_length);
        let clamped = distance.clamp(0., length);
        let sample = if let Some(lookup) = &self.lookup {
            self.curve()
                .evaluate_with_tangent(lookup.parameter(clamped / length))?
        } else {
            self.length.sample_at_distance(clamped)?
        };
        let extra = distance - clamped;
        let origin = sample
            .point()
            .translated(sample.tangent().as_vector().scaled(extra)?)?;
        let normal_point = if self.surface.is_some() {
            // Surface orientation follows the axial endpoint residual too,
            // before applying either transverse coordinate.
            Some(origin.translated(sample.tangent().as_vector().scaled(along)?)?)
        } else {
            None
        };
        Ok(self
            .frame_at_point(sample.parameter(), normal_point)?
            .with_origin(origin))
    }
    fn total_length(&self) -> Real {
        self.lookup
            .as_ref()
            .map_or(self.length.total_length(), |q| q.length)
    }
    fn distance_at_parameter(&self, parameter: Real) -> Result<Real, GeometryError> {
        if let Some(lookup) = &self.lookup {
            Ok(lookup.fraction(parameter) * lookup.mapped_length)
        } else {
            self.length.distance_at_parameter(parameter)
        }
    }
}

/// Curve-to-curve point map with supporting-plane or transported frames.
/// Spatial transport is a mathematical frame policy; native Flow's spatial
/// automatic direction policy is still under investigation. Explicit up axes
/// provide spatial Roadlike sections. NURBS guides retain an independently
/// generated normalized-length lookup in both directions. Clones share guide
/// geometry, arc-length brackets and frame checkpoints. Source coordinates
/// and adaptive fitting retain the existing kernel's finite/error contracts.
#[derive(Clone, Debug)]
pub struct CurveFlowPointMorph {
    base: Arc<Guide>,
    target: Arc<Guide>,
    options: CurveFlowOptions,
    linear_mapping: Option<crate::AffineTransform3>,
    interpolation_solver: Option<MorphInterpolationSolver>,
}
impl CurveFlowPointMorph {
    pub fn try_new(
        base: Curve3,
        target: Curve3,
        options: CurveFlowOptions,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        Self::try_new_shared(Arc::new(base), Arc::new(target), options, tolerance)
    }
    pub fn try_new_shared(
        base: Arc<Curve3>,
        target: Arc<Curve3>,
        options: CurveFlowOptions,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        Self::try_new_shared_with_surface(base, target, options, None, tolerance)
    }
    pub fn try_new_shared_with_surface(
        base: Arc<Curve3>,
        target: Arc<Curve3>,
        options: CurveFlowOptions,
        surface: Option<CurveFlowTargetSurface>,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        let mut base = Guide::new(base, options.reverse_base, options.base_up, None, tolerance)?;
        let mut target = Guide::new(
            target,
            options.reverse_target,
            options.target_up,
            surface,
            tolerance,
        )?;
        let base_linear = base.curve().to_nurbs()?.is_linear_at_zero_tolerance()?;
        let target_linear = target.curve().to_nurbs()?.is_linear_at_zero_tolerance()?;
        if !options.stretch
            && base.total_length() < target.total_length()
            && let Some(lookup) = &target.lookup
        {
            target.lookup = Some(lookup.prefix(&target.length, base.total_length(), tolerance)?);
        }
        if target.surface.is_none() && options.base_up.is_none() && options.target_up.is_none() {
            // A straight rail has no unique supporting plane. Borrow the
            // curved rail's plane; a parallel tangent needs a deterministic
            // perpendicular axis instead. Both straight rails use the direct
            // shortest rotation below.
            fn borrowed_normal(
                guide: &Guide,
                normal: UnitVector3,
            ) -> Result<UnitVector3, GeometryError> {
                let t = guide
                    .curve()
                    .evaluate_with_tangent(*guide.curve().domain().start())?
                    .tangent();
                if normal.as_vector().cross(t.as_vector())?.length()? > 64. * Real::EPSILON {
                    Ok(normal)
                } else {
                    Ok(Frame3::try_from_normal(
                        guide.curve().start_point()?,
                        t.as_vector(),
                        Tolerance::NUMERICAL_VALIDATION,
                    )?
                    .x_axis())
                }
            }
            if base_linear
                && !target_linear
                && let Some(n) = target.normal
            {
                base.normal = Some(borrowed_normal(&base, n)?);
            } else if target_linear
                && !base_linear
                && let Some(n) = base.normal
            {
                target.normal = Some(borrowed_normal(&target, n)?);
            } else if !base_linear
                && !target_linear
                && let (Some(a), Some(b)) = (base.normal, target.normal)
                && a.as_vector().dot(b.as_vector())? < 0.
            {
                target.normal = Some(b.opposite());
            }
        }
        if target.surface.is_some() && base_linear && options.base_up.is_none() {
            let tangent = base
                .curve()
                .evaluate_with_tangent(*base.curve().domain().start())?
                .tangent();
            let vertical = Vector3::try_new(0., 0., 1.)?.normalized_nonzero()?;
            base.normal = Some(
                if vertical.as_vector().cross(tangent.as_vector())?.length()? == 0. {
                    Frame3::try_from_normal(
                        base.curve().start_point()?,
                        tangent.as_vector(),
                        Tolerance::NUMERICAL_VALIDATION,
                    )?
                    .x_axis()
                } else {
                    vertical
                },
            );
        }
        let base = Arc::new(base);
        let target = Arc::new(target);
        let linear_mapping = if target.surface.is_none()
            && options.base_up.is_none()
            && options.target_up.is_none()
            && base_linear
            && target_linear
        {
            let source_origin = base.curve().start_point()?;
            let target_origin = target.curve().start_point()?;
            let source_direction = base
                .curve()
                .evaluate_with_tangent(*base.curve().domain().start())?
                .tangent();
            let target_direction = target
                .curve()
                .evaluate_with_tangent(*target.curve().domain().start())?
                .tangent();
            let axial = if options.stretch {
                crate::parameter::scaled_ratio(
                    1.,
                    target.length.total_length(),
                    base.length.total_length(),
                )?
            } else {
                1.
            };
            let opposite = source_direction
                .as_vector()
                .dot(target_direction.as_vector())?
                < 0.
                && source_direction
                    .as_vector()
                    .cross(target_direction.as_vector())?
                    .length()?
                    == 0.;
            if opposite {
                // The separated lines determine the half-turn plane. Exact
                // collinearity uses the existing OpenNURBS-style perpendicular.
                let chord = source_origin.vector_to(base.curve().end_point()?)?;
                let between = source_origin.vector_to(target_origin).or_else(|_| {
                    let a = source_origin.to_array();
                    let b = target_origin.to_array();
                    let scale = a.into_iter().chain(b).map(Real::abs).fold(0., Real::max);
                    Vector3::try_from(std::array::from_fn(|i| b[i] / scale - a[i] / scale))
                })?;
                let normal = chord.cross(between).or_else(|_| {
                    chord
                        .normalized_nonzero()?
                        .as_vector()
                        .cross(between.normalized_nonzero()?.as_vector())
                })?;
                let normal = if normal.to_array() == [0.; 3] {
                    Frame3::try_from_normal(
                        source_origin,
                        source_direction.as_vector(),
                        Tolerance::NUMERICAL_VALIDATION,
                    )?
                    .x_axis()
                    .as_vector()
                } else {
                    normal.normalized_nonzero()?.as_vector()
                };
                let source = Frame3::try_from_directions(
                    source_origin,
                    source_direction.as_vector(),
                    normal.cross(source_direction.as_vector())?,
                    Tolerance::NUMERICAL_VALIDATION,
                )?;
                let target = Frame3::try_from_directions(
                    target_origin,
                    target_direction.as_vector(),
                    normal.cross(target_direction.as_vector())?,
                    Tolerance::NUMERICAL_VALIDATION,
                )?;
                Some(crate::AffineTransform3::try_frame_mapping(
                    source,
                    target,
                    [axial, 1., 1.],
                )?)
            } else {
                Some(crate::AffineTransform3::try_direction_mapping(
                    source_origin,
                    source_direction,
                    target_origin,
                    target_direction,
                    axial,
                    1.,
                    Tolerance::MESH_VALIDATION,
                )?)
            }
        } else {
            None
        };
        Ok(Self {
            base,
            target,
            options,
            linear_mapping,
            interpolation_solver: None,
        })
    }
    pub fn with_interpolation_solver(mut self, solver: MorphInterpolationSolver) -> Self {
        self.interpolation_solver = Some(solver);
        self
    }
    pub fn rigid_transform(
        &self,
        center: Point3,
    ) -> Result<crate::AffineTransform3, GeometryError> {
        match super::rigid_transform(self, center) {
            // A Roadlike pole can make a forward placement frame singular.
            // Native Rigid then retains the object's original pose, including
            // its origin. Other numerical failures still propagate.
            Err(GeometryError::Degenerate { .. })
                if self.options.base_up.is_some() || self.options.target_up.is_some() =>
            {
                Ok(crate::AffineTransform3::identity())
            }
            result => result,
        }
    }
    pub fn uses_spatial_transport(&self) -> bool {
        self.linear_mapping.is_none()
            && (self.base.normal.is_none()
                || (self.target.normal.is_none() && self.target.surface.is_none()))
    }
    pub fn with_preserve_structure(mut self, preserve: bool) -> Self {
        self.options.preserve_structure = preserve;
        self
    }
    pub fn options(&self) -> CurveFlowOptions {
        self.options
    }
    pub fn base_length(&self) -> Real {
        self.base.total_length()
    }
    pub fn target_length(&self) -> Real {
        self.target.total_length()
    }
}
impl PointMorph for CurveFlowPointMorph {
    fn interpolation_solver(&self) -> Option<MorphInterpolationSolver> {
        self.interpolation_solver
    }
    fn morph_point(&self, point: Point3) -> Result<Point3, GeometryError> {
        let parameter = if self.linear_mapping.is_none() || self.options.local_radii.is_some() {
            Some(
                self.base
                    .curve()
                    .closest_parameter(point, self.base.tolerance)?,
            )
        } else {
            None
        };
        let weight = parameter
            .map(|t| self.local_weight(point, t))
            .transpose()?
            .unwrap_or(1.);
        if weight == 0. {
            return Ok(point);
        }
        let mapped = if let Some(map) = self.linear_mapping {
            map.transform_point(point)?
        } else {
            self.flow_point_at_parameter(point, parameter.unwrap())?
        };
        if let Some(radii) = self.options.local_radii {
            radii.blend(point, mapped, weight)
        } else {
            Ok(mapped)
        }
    }
    fn morph_nurbs_curve(
        &self,
        curve: &NurbsCurve,
        tolerance: Tolerance,
    ) -> Result<NurbsCurve, GeometryError> {
        if self.options.preserve_structure {
            self.morph_nurbs_curve_controls(curve)
        } else {
            super::fit_curve(self, curve, tolerance)
        }
    }
    fn morph_nurbs_surface(
        &self,
        surface: &NurbsSurface,
        tolerance: Tolerance,
    ) -> Result<NurbsSurface, GeometryError> {
        if self.options.preserve_structure {
            self.morph_nurbs_surface_controls(surface)
        } else {
            super::fit_surface(self, surface, tolerance)
        }
    }
}
impl CurveFlowPointMorph {
    fn flow_point_at_parameter(
        &self,
        point: Point3,
        parameter: Real,
    ) -> Result<Point3, GeometryError> {
        let source = self.base.frame(parameter)?;
        let offset = source.origin.vector_to(point)?;
        let along = offset.dot(source.axes[0])?;
        let y = offset.dot(source.axes[1])?;
        let z = offset.dot(source.axes[2])?;
        let mut distance = self.base.distance_at_parameter(parameter)?;
        let mut along = along;
        if self.options.stretch {
            distance =
                crate::parameter::scaled_ratio(distance, self.target_length(), self.base_length())?;
            along =
                crate::parameter::scaled_ratio(along, self.target_length(), self.base_length())?;
        }
        crate::require_finite([distance, along, y, z], "curve flow coordinates")?;
        let frame = self.target.at_distance(distance, along)?;
        let c = frame.axes[0].scaled(along)?.to_array();
        let a = frame.axes[1].scaled(y)?.to_array();
        let b = frame.axes[2].scaled(z)?.to_array();
        frame
            .origin
            .translated(Vector3::try_from(std::array::from_fn(|i| {
                a[i] + b[i] + c[i]
            }))?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn p(x: Real, y: Real, z: Real) -> Point3 {
        Point3::try_new(x, y, z).unwrap()
    }
    fn guides() -> (Curve3, Curve3) {
        let base =
            crate::LineSegment::try_new(p(0., 0., 0.), p(10., 0., 0.), Tolerance::DEFAULT).unwrap();
        let circle = crate::Circle3::try_new(
            p(20., 0., 0.),
            10.,
            Vector3::try_new(0., 0., 1.)
                .unwrap()
                .normalized_nonzero()
                .unwrap(),
            Tolerance::DEFAULT,
        )
        .unwrap();
        (
            Curve3::Line(base),
            Curve3::Arc(
                crate::CircularArc3::try_from_circle_sweep(circle, std::f64::consts::FRAC_PI_2)
                    .unwrap(),
            ),
        )
    }
    #[test]
    fn surface_sections_override_target_up_respect_face_orientation_and_share_storage() {
        let surface = Arc::new(
            NurbsSurface::try_bilinear([
                p(18., 0., -4.),
                p(32., 0., -4.),
                p(32., 0., 4.),
                p(18., 0., 4.),
            ])
            .unwrap(),
        );
        for reversed in [false, true] {
            let morph = CurveFlowPointMorph::try_new_shared_with_surface(
                Arc::new(Curve3::Line(
                    crate::LineSegment::try_new(p(0., 0., 0.), p(10., 0., 0.), Tolerance::DEFAULT)
                        .unwrap(),
                )),
                Arc::new(Curve3::Line(
                    crate::LineSegment::try_new(p(20., 0., 0.), p(30., 0., 0.), Tolerance::DEFAULT)
                        .unwrap(),
                )),
                CurveFlowOptions {
                    target_up: Some(Vector3::try_new(0., 0., 1.).unwrap()),
                    ..Default::default()
                },
                Some(CurveFlowTargetSurface::new(surface.clone(), reversed)),
                Tolerance::DEFAULT,
            )
            .unwrap();
            assert!(!morph.uses_spatial_transport());
            assert!(morph.linear_mapping.is_none());
            let clone = morph.clone();
            assert!(Arc::ptr_eq(&morph.target, &clone.target));
            assert!(Arc::ptr_eq(
                &surface,
                &morph.target.surface.as_ref().unwrap().surface
            ));
            let sign = if reversed { -1. } else { 1. };
            for x in [-1., 0., 4., 10., 11.] {
                assert!(
                    morph
                        .morph_point(p(x, 1., 2.))
                        .unwrap()
                        .distance_to(p(x + 20., -2. * sign, sign))
                        .unwrap()
                        < 1e-12
                );
            }
        }
    }
    #[test]
    fn flow_endpoint_residual_uses_endpoint_tangent_and_clones_share_preparation() {
        let (a, b) = guides();
        for stretch in [false, true] {
            let morph = CurveFlowPointMorph::try_new(
                a.clone(),
                b.clone(),
                CurveFlowOptions {
                    stretch,
                    ..Default::default()
                },
                Tolerance::DEFAULT,
            )
            .unwrap();
            let clone = morph.clone();
            assert!(Arc::ptr_eq(&morph.base, &clone.base));
            assert!(Arc::ptr_eq(&morph.target, &clone.target));
            let scale = if stretch {
                std::f64::consts::FRAC_PI_2
            } else {
                1.
            };
            // Source -1 stays on the target's start tangent instead of bending backwards.
            let actual = morph.morph_point(p(-1., 2., 3.)).unwrap();
            assert!(actual.distance_to(p(28., -scale, 3.)).unwrap() < 1e-12);
            let angle = scale;
            let expected = p(
                20. + 8. * angle.cos() - scale * angle.sin(),
                8. * angle.sin() + scale * angle.cos(),
                3.,
            );
            assert!(
                morph
                    .morph_point(p(11., 2., 3.))
                    .unwrap()
                    .distance_to(expected)
                    .unwrap()
                    < 1e-11
            );
        }
    }
    #[test]
    fn flow_preserved_rational_controls_keep_degree_knots_and_weights() {
        let (a, b) = guides();
        let morph = CurveFlowPointMorph::try_new(
            a,
            b,
            CurveFlowOptions {
                preserve_structure: true,
                ..Default::default()
            },
            Tolerance::DEFAULT,
        )
        .unwrap();
        let source = NurbsCurve::try_new_rational(
            2,
            vec![
                crate::WeightedPoint3::try_new(p(1., 0., 0.), 1.).unwrap(),
                crate::WeightedPoint3::try_new(p(4., 2., 1.), 0.7).unwrap(),
                crate::WeightedPoint3::try_new(p(8., 0., 0.), 1.).unwrap(),
            ],
            vec![0., 0., 0., 1., 1., 1.],
        )
        .unwrap();
        let out = morph
            .morph_nurbs_curve(&source, Tolerance::DEFAULT)
            .unwrap();
        assert_eq!(out.degree(), source.degree());
        assert_eq!(out.knots(), source.knots());
        for (a, b) in source.control_points().iter().zip(out.control_points()) {
            assert_eq!(a.weight(), b.weight());
            assert_eq!(morph.morph_point(a.point()).unwrap(), b.point());
        }
    }
}

#[cfg(test)]
mod line_orientation_tests {
    use super::*;
    fn p(x: Real, y: Real, z: Real) -> Point3 {
        Point3::try_new(x, y, z).unwrap()
    }
    fn line(a: Point3, b: Point3, t: Tolerance) -> Curve3 {
        Curve3::Line(crate::LineSegment::try_new(a, b, t).unwrap())
    }
    #[test]
    fn opposite_lines_use_their_common_plane_and_collinear_lines_use_a_deterministic_axis() {
        let t = Tolerance::DEFAULT;
        for (origin, expected_y, expected_z) in [
            (
                p(15., 2., 3.),
                p(15., 2. + 5. / 13., 3. - 12. / 13.),
                p(15., 2. - 12. / 13., 3. - 5. / 13.),
            ),
            (p(15., 0., 0.), p(15., 1., 0.), p(15., 0., -1.)),
        ] {
            let m = CurveFlowPointMorph::try_new(
                line(p(0., 0., 0.), p(10., 0., 0.), t),
                line(
                    origin,
                    origin
                        .translated(Vector3::try_new(-20., 0., 0.).unwrap())
                        .unwrap(),
                    t,
                ),
                CurveFlowOptions::default(),
                t,
            )
            .unwrap();
            assert!(
                m.morph_point(p(0., 1., 0.))
                    .unwrap()
                    .distance_to(expected_y)
                    .unwrap()
                    < 1e-12
            );
            assert!(
                m.morph_point(p(0., 0., 1.))
                    .unwrap()
                    .distance_to(expected_z)
                    .unwrap()
                    < 1e-12
            );
        }
    }
    #[test]
    fn opposite_guide_origins_can_exceed_a_finite_subtraction_without_losing_a_finite_map() {
        let t = Tolerance::MESH_VALIDATION;
        let m = CurveFlowPointMorph::try_new(
            line(p(1e308, 0., 0.), p(1e308, 10., 0.), t),
            line(p(-1e308, 0., 0.), p(-1e308, -20., 0.), t),
            CurveFlowOptions::default(),
            t,
        )
        .unwrap();
        assert_eq!(m.morph_point(p(1e308, 1., 0.)).unwrap(), p(-1e308, -2., 0.));
    }
}

#[cfg(test)]
mod pole_tests {
    use super::*;
    fn p(x: Real, y: Real, z: Real) -> Point3 {
        Point3::try_new(x, y, z).unwrap()
    }
    fn vector(x: Real, y: Real, z: Real) -> Vector3 {
        Vector3::try_new(x, y, z).unwrap()
    }
    fn morph(up: Vector3) -> CurveFlowPointMorph {
        CurveFlowPointMorph::try_new(
            Curve3::Line(
                crate::LineSegment::try_new(p(0., 0., 0.), p(10., 0., 0.), Tolerance::DEFAULT)
                    .unwrap(),
            ),
            Curve3::Line(
                crate::LineSegment::try_new(p(20., 30., 0.), p(20., 50., 0.), Tolerance::DEFAULT)
                    .unwrap(),
            ),
            CurveFlowOptions {
                base_up: Some(up),
                target_up: Some(vector(0., 0., 1.)),
                ..Default::default()
            },
            Tolerance::DEFAULT,
        )
        .unwrap()
    }
    #[test]
    fn flow_axial_section_collapses_transverse_coordinates_and_keeps_endpoint_residuals() {
        let m = morph(vector(1., 0., 0.));
        for x in [-1., 0., 3., 10., 11.] {
            for (y, z) in [(0., 0.), (1., 2.), (-3., 4.)] {
                assert_eq!(m.morph_point(p(x, y, z)).unwrap(), p(20., 30. + 2. * x, 0.));
            }
        }
        let pose = m.rigid_transform(p(5., 1., 2.)).unwrap();
        assert_eq!(pose, crate::AffineTransform3::identity());
        assert!(m.rigid_transform(p(f64::MAX, 1., 2.)).is_err());
    }
    #[test]
    fn flow_nonzero_cross_products_retain_small_direction_changes() {
        let m = morph(vector(1., 0., 1e-100));
        let a = m.morph_point(p(5., 0., 0.)).unwrap();
        let b = m.morph_point(p(5., 1., 0.)).unwrap();
        assert!((a.distance_to(b).unwrap() - 1.).abs() < 1e-14);
    }
}
