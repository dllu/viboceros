//! SubD limit-mesh density, independent of control topology and color mapping.
#[cfg(test)]
mod tests;

/// Public OpenNURBS/Rhino SubD display parameters. Adaptive density reduces
/// subdivision for large control nets using the native 512,000-quad budget.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SubDMeshDensity {
    level: u8,
    adaptive: bool,
}

impl Default for SubDMeshDensity {
    fn default() -> Self {
        Self {
            level: 4,
            adaptive: true,
        }
    }
}

impl SubDMeshDensity {
    pub const ADAPTIVE_QUAD_MAXIMUM: u64 = 512_000;

    /// The native API accepts levels 0–6; Rhino's slider exposes levels 1–5.
    pub const fn new(level: u8, adaptive: bool) -> Option<Self> {
        if level <= 6 {
            Some(Self { level, adaptive })
        } else {
            None
        }
    }
    pub const fn level(self) -> u8 {
        self.level
    }
    pub const fn is_adaptive(self) -> bool {
        self.adaptive
    }

    /// Matches ON_SubDDisplayParameters::AbsoluteDisplayDensityFromSubDFaceCount.
    /// Count in 64 bits to avoid overflow on large control nets.
    pub fn effective_level(self, face_count: usize) -> u32 {
        let mut level = u32::from(self.level);
        if self.adaptive {
            level = level.max(1);
            while level > 1 && face_count as u64 > Self::ADAPTIVE_QUAD_MAXIMUM / 4u64.pow(level) {
                level -= 1;
            }
        }
        level
    }

    pub(super) fn for_subd(self, subd: &super::SubD) -> u32 {
        let level = self.effective_level(subd.faces().len());
        // Nonquad analysis fragments need one subdivision to produce quads.
        // OpenNURBS also imposes this minimum for faces with >4 edges.
        if level == 0 && subd.faces().iter().any(|face| face.len() != 4) {
            1
        } else {
            level
        }
    }
}
