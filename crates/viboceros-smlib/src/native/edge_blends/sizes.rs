//! Size profiles on original edges in a single native blend solve.
use super::*;

#[derive(Clone, Copy)]
pub enum EdgeBlendProfile {
    Fillet,
    Chamfer(EdgeBlendRailType),
    Curvature(EdgeBlendRailType),
}

#[derive(Clone, Debug, PartialEq)]
pub struct EdgeBlendSizes {
    pub ends: [f64; 2],
    /// Additional sizes in increasing normalized curve-domain order.
    pub interior: Vec<(f64, f64)>,
}

impl From<[f64; 2]> for EdgeBlendSizes {
    fn from(ends: [f64; 2]) -> Self {
        Self {
            ends,
            interior: Vec::new(),
        }
    }
}

impl EdgeBlendSizes {
    pub fn stations(&self) -> impl Iterator<Item = (f64, f64)> + '_ {
        std::iter::once((0., self.ends[0]))
            .chain(self.interior.iter().copied())
            .chain(std::iter::once((1., self.ends[1])))
    }

    pub fn is_valid(&self) -> bool {
        let mut prior = f64::NEG_INFINITY;
        self.stations().all(|(parameter, size)| {
            let ordered = parameter > prior;
            prior = parameter;
            ordered && size.is_finite() && size > 0.
        })
    }

    pub fn all_equal_to(&self, size: f64) -> bool {
        self.stations().all(|(_, value)| value == size)
    }
}

unsafe extern "C" {
    fn vb_imported_solid_blend_edge_sizes(
        solid: *mut NativeSolid,
        indices: *const usize,
        sizes: *const f64,
        parameters: *const f64,
        offsets: *const usize,
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
        let edges = edges
            .iter()
            .map(|&(index, sizes)| (index, sizes.into()))
            .collect::<Vec<_>>();
        Self::blend_brep_edge_profiles(brep, &edges, profile, tolerance, local_merge)
    }

    pub fn blend_brep_edge_profiles(
        brep: &Brep,
        edges: &[(usize, EdgeBlendSizes)],
        profile: EdgeBlendProfile,
        tolerance: Tolerance,
        local_merge: bool,
    ) -> Result<Brep, Error> {
        if edges.is_empty()
            || edges.len() > 100000
            || edges
                .iter()
                .any(|(index, sizes)| *index >= brep.edges().len() || !sizes.is_valid())
        {
            return Err(Error::Kernel("Invalid per-edge blend parameters".into()));
        }
        let indices = edges.iter().map(|(edge, _)| *edge).collect::<Vec<_>>();
        let mut offsets = Vec::with_capacity(edges.len() + 1);
        offsets.push(0);
        let mut parameters = Vec::new();
        let mut sizes = Vec::new();
        for (_, profile) in edges {
            for (parameter, size) in profile.stations() {
                parameters.push(parameter);
                sizes.push(size);
            }
            offsets.push(sizes.len());
        }
        let minimum = sizes.iter().copied().fold(f64::INFINITY, f64::min);
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
                parameters.as_ptr(),
                offsets.as_ptr(),
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
