//! Size pairs on original edges in a single native blend solve.
use super::*;

#[derive(Clone, Copy)]
pub enum EdgeBlendProfile {
    Fillet,
    Chamfer(EdgeBlendRailType),
    Curvature(EdgeBlendRailType),
}

unsafe extern "C" {
    fn vb_imported_solid_blend_edge_sizes(
        solid: *mut NativeSolid,
        indices: *const usize,
        sizes: *const f64,
        end_sizes: *const f64,
        count: usize,
        minimum: f64,
        accuracy: f64,
        profile: c_int,
        rail: c_int,
        local_merge: c_int,
        error: *mut c_char,
        capacity: usize,
    ) -> c_int;
}

impl Solid {
    pub fn blend_brep_edge_sizes(
        brep: &Brep,
        edges: &[(usize, [f64; 2])],
        profile: EdgeBlendProfile,
        tolerance: Tolerance,
        local_merge: bool,
    ) -> Result<Brep, Error> {
        if edges.is_empty()
            || edges.len() > 100000
            || edges.iter().any(|&(index, sizes)| {
                index >= brep.edges().len()
                    || sizes
                        .into_iter()
                        .any(|size| !size.is_finite() || size <= 0.)
            })
        {
            return Err(Error::Kernel("Invalid per-edge blend parameters".into()));
        }
        let indices = edges.iter().map(|(edge, _)| *edge).collect::<Vec<_>>();
        let (sizes, end_sizes): (Vec<_>, Vec<_>) = edges.iter().map(|&(_, [a, b])| (a, b)).unzip();
        let minimum = sizes
            .iter()
            .chain(&end_sizes)
            .copied()
            .fold(f64::INFINITY, f64::min);
        let (profile_code, rail) = match profile {
            EdgeBlendProfile::Fillet => (0, EdgeBlendRailType::RollingBall),
            EdgeBlendProfile::Chamfer(rail) => (1, rail),
            EdgeBlendProfile::Curvature(rail) => (2, rail),
        };
        let working = if matches!(profile, EdgeBlendProfile::Fillet) {
            tolerance
        } else {
            Tolerance::try_new(
                tolerance.absolute().min(minimum * 1e-9),
                tolerance.relative(),
                tolerance.angular(),
            )?
        };
        let solid = Self::from_brep(brep, working)?;
        let accuracy = (minimum * 1e-10).min(tolerance.absolute() * 0.1);
        call(|error, capacity| unsafe {
            vb_imported_solid_blend_edge_sizes(
                solid.handle.as_ptr(),
                indices.as_ptr(),
                sizes.as_ptr(),
                end_sizes.as_ptr(),
                indices.len(),
                minimum,
                accuracy,
                profile_code,
                c_int::from(rail == EdgeBlendRailType::RollingBall),
                c_int::from(local_merge),
                error,
                capacity,
            )
        })?;
        if local_merge {
            solid.to_brep_with_stitched_uv_joints(tolerance)
        } else {
            solid.to_brep(tolerance)
        }
    }
}
