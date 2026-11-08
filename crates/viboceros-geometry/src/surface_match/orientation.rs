use super::*;

pub(super) struct Chart {
    swapped: bool,
    reversed: bool,
}
impl Chart {
    pub fn new(side: SurfaceExtensionEdge, at_end: bool) -> Self {
        let swapped = matches!(
            side,
            SurfaceExtensionEdge::South | SurfaceExtensionEdge::North
        );
        let high = matches!(
            side,
            SurfaceExtensionEdge::East | SurfaceExtensionEdge::North
        );
        Self {
            swapped,
            reversed: high != at_end,
        }
    }
    pub fn orient(&self, s: &NurbsSurface) -> Result<NurbsSurface, GeometryError> {
        let s = if self.swapped {
            s.try_swapped_uv()?
        } else {
            s.clone()
        };
        if self.reversed {
            s.try_reversed_u()
        } else {
            Ok(s)
        }
    }
    pub fn restore(&self, s: &NurbsSurface) -> Result<NurbsSurface, GeometryError> {
        let s = if self.reversed {
            s.try_reversed_u()?
        } else {
            s.clone()
        };
        if self.swapped {
            s.try_swapped_uv()
        } else {
            Ok(s)
        }
    }
}
pub(super) fn edge_forward(side: SurfaceExtensionEdge) -> bool {
    matches!(
        side,
        SurfaceExtensionEdge::East | SurfaceExtensionEdge::South
    )
}
