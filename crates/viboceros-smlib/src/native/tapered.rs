//! Draft sweeps delegate offsets, corner construction and topology to SMLib.
use super::*;

unsafe extern "C" {
    fn vb_shape_tapered_extrusion(
        curve: *const NativeCurve,
        direction: *const f64,
        height: f64,
        angle_degrees: f64,
        corners: c_int,
        cap_ends: c_int,
        oblique: c_int,
        tolerance: f64,
        cuts: *const f64,
        cut_count: usize,
        out: *mut *mut NativeSolid,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
}

/// How the draft joins adjacent offset segments at an expanding corner.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(i32)]
pub enum TaperCorner {
    #[default]
    Sharp = 0,
    Round = 1,
    Smooth = 2,
}

impl From<bool> for TaperCorner {
    fn from(round: bool) -> Self {
        if round { Self::Round } else { Self::Sharp }
    }
}

pub(super) fn oblique_axis(
    direction: viboceros_geometry::Vector3,
    plane_normal: Option<viboceros_geometry::Vector3>,
    tolerance: viboceros_geometry::Tolerance,
) -> Result<bool, Error> {
    let Some(normal) = plane_normal else {
        return Ok(false);
    };
    if direction.dot(normal)?.abs() <= tolerance.angular() {
        return Err(Error::Kernel(
            "The taper direction lies in the profile plane".into(),
        ));
    }
    Ok(direction.cross(normal)?.length()? > tolerance.angular())
}

impl Shape {
    /// Extrude a connected planar profile with a signed draft angle in degrees.
    /// Positive angles shrink a closed profile; negative angles expand it.
    /// Round uses circular fillets and Smooth uses cubic tangent joins.
    /// A planar profile swept obliquely has independent offset walls and can
    /// retain a base cap. Normal capped sweeps must form a manifold solid.
    pub fn tapered_extrusion(
        profile: &KernelCurve,
        displacement: viboceros_geometry::Vector3,
        angle_degrees: f64,
        corners: impl Into<TaperCorner>,
        cap_ends: bool,
        tolerance: viboceros_geometry::Tolerance,
    ) -> Result<Self, Error> {
        Self::tapered_extrusion_impl(
            profile,
            displacement,
            angle_degrees,
            corners.into(),
            cap_ends,
            tolerance,
            None,
            false,
        )
    }

    /// Draft a planar profile using only the supplied, strictly interior
    /// partition parameters. An empty slice keeps the profile intact through
    /// the offset operation. Zero draft uses the ordinary linear sweep.
    pub fn tapered_extrusion_partitioned(
        profile: &KernelCurve,
        displacement: viboceros_geometry::Vector3,
        angle_degrees: f64,
        corners: TaperCorner,
        cap_ends: bool,
        tolerance: viboceros_geometry::Tolerance,
        cuts: &[f64],
    ) -> Result<Self, Error> {
        Self::tapered_extrusion_impl(
            profile,
            displacement,
            angle_degrees,
            corners,
            cap_ends,
            tolerance,
            Some(cuts),
            false,
        )
    }

    /// Build independent oblique draft walls using the input traversal to
    /// choose the offset side, including when a closed input supplies a base.
    pub fn directed_oblique_taper(
        profile: &KernelCurve,
        displacement: viboceros_geometry::Vector3,
        angle_degrees: f64,
        cap_base: bool,
        tolerance: viboceros_geometry::Tolerance,
    ) -> Result<Self, Error> {
        Self::tapered_extrusion_impl(
            profile,
            displacement,
            angle_degrees,
            TaperCorner::Sharp,
            cap_base,
            tolerance,
            None,
            true,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn tapered_extrusion_impl(
        profile: &KernelCurve,
        displacement: viboceros_geometry::Vector3,
        angle_degrees: f64,
        corners: TaperCorner,
        cap_ends: bool,
        tolerance: viboceros_geometry::Tolerance,
        cuts: Option<&[f64]>,
        directed: bool,
    ) -> Result<Self, Error> {
        if !angle_degrees.is_finite() || angle_degrees.abs() >= 90. {
            return Err(Error::Kernel(
                "Draft angle must be finite and between -90 and 90 degrees".into(),
            ));
        }
        if angle_degrees == 0. {
            return Self::linear_sweep(&[profile], displacement, cap_ends, tolerance);
        }
        let height = displacement.length()?;
        let direction = displacement.normalized_nonzero()?.as_vector();
        let mode = if directed {
            2
        } else {
            c_int::from(oblique_axis(
                direction,
                KernelCurve::parallel_plane_normal(&[profile], tolerance)?,
                tolerance,
            )?)
        };
        let direction = direction.to_array();
        let (cuts, cut_count) =
            cuts.map_or((std::ptr::null(), 0), |cuts| (cuts.as_ptr(), cuts.len()));
        let mut pointer = std::ptr::null_mut();
        call(|error, capacity| unsafe {
            vb_shape_tapered_extrusion(
                profile.handle.as_ptr(),
                direction.as_ptr(),
                height,
                angle_degrees,
                corners as c_int,
                c_int::from(cap_ends),
                mode,
                tolerance.absolute(),
                cuts,
                cut_count,
                &mut pointer,
                error,
                capacity,
            )
        })?;
        Ok(Self {
            model: Solid {
                handle: NonNull::new(pointer).ok_or(Error::InvalidOutput)?,
                _thread_local: PhantomData,
            },
        })
    }
}
