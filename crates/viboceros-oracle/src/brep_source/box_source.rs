//! Preserve the original shared box input's UV definitions in saved replays.
//!
//! The kernel's box constructor follows Rhino's natural wall axes. The older
//! shared-input recipe used a cyclically shifted UV origin on two walls;
//! Rhino received those native-written 3DM inputs during the archived probes.
//! Keep that recipe here, before commands run, rather than rewriting outputs.
use super::*;
use viboceros_geometry::{BrepLoop, BrepTrim, Point2};

pub(super) fn build(
    min: [f64; 3],
    max: [f64; 3],
    tolerance: Tolerance,
) -> Result<Brep, ProbeError> {
    let brep = Brep::try_box(
        viboceros_command::CommandContext::default().construction_plane,
        std::array::from_fn(|i| [min[i], max[i]]),
        tolerance,
    )?;
    let mut faces = brep.faces().to_vec();
    let uv = [
        Point2::try_new(0., 0.)?,
        Point2::try_new(1., 0.)?,
        Point2::try_new(1., 1.)?,
        Point2::try_new(0., 1.)?,
    ];
    let iso = [
        SurfaceIso::South,
        SurfaceIso::East,
        SurfaceIso::North,
        SurfaceIso::West,
    ];
    for (face, corners, edges) in [
        (3, [2, 6, 7, 3], [10, 6, 11, 2]),
        (4, [0, 4, 6, 2], [8, 7, 10, 3]),
    ] {
        let surface = NurbsSurface::try_bilinear(corners.map(|v| brep.vertices()[v].point()))?;
        let trims = (0..4)
            .map(|side| {
                BrepTrim::try_new(
                    [corners[side], corners[(side + 1) % 4]],
                    Some(edges[side]),
                    side >= 2,
                    NurbsCurve2::try_line(uv[side], uv[(side + 1) % 4])?,
                    BrepTrimType::Mated,
                    iso[side],
                    [0., 0.],
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        faces[face] = BrepFace::try_new(
            surface,
            false,
            vec![BrepLoop::try_new(BrepLoopType::Outer, trims)?],
        )?;
    }
    Ok(Brep::try_new(
        brep.vertices().to_vec(),
        brep.edges().to_vec(),
        faces,
        tolerance,
    )?)
}
