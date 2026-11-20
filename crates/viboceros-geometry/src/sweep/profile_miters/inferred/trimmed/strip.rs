//! A sweep leg with natural or transverse boundaries on its extended support.
use super::*;
use crate::{
    BrepEdge, BrepFace, BrepLoop, BrepLoopType, BrepTrim, BrepTrimType, BrepVertex, SurfaceIso,
};

pub(super) struct Part {
    pub brep: Brep,
    pub sides: [Vec<usize>; 2],
}

pub(super) struct Cut {
    pub spatial: NurbsCurve,
    pub uv: NurbsCurve2,
}

impl Cut {
    pub(super) fn at_u(
        surface: &NurbsSurface,
        u: Real,
        spatial: NurbsCurve,
    ) -> Result<Self, GeometryError> {
        let v = surface.domain_v();
        Ok(Self {
            spatial,
            uv: NurbsCurve2::try_line(
                Point2::try_new(u, *v.start())?,
                Point2::try_new(u, *v.end())?,
            )?,
        })
    }
}

// Positive weights bound the whole graph by its control ordinates. Keep the
// extensions if the two cuts on a leg cannot be separated this way.
pub(super) fn ordered(surface: &NurbsSurface, west: Option<&Cut>, east: Option<&Cut>) -> bool {
    let u = surface.domain_u();
    let max_west = west.map_or(*u.start(), |cut| {
        cut.uv
            .control_points()
            .iter()
            .map(|c| c.point().x())
            .fold(Real::NEG_INFINITY, Real::max)
    });
    let min_east = east.map_or(*u.end(), |cut| {
        cut.uv
            .control_points()
            .iter()
            .map(|c| c.point().x())
            .fold(Real::INFINITY, Real::min)
    });
    max_west < min_east
}

pub(super) fn build(
    surface: NurbsSurface,
    [west, east]: [Option<Cut>; 2],
    original: RangeInclusive<Real>,
    tolerance: Tolerance,
) -> Result<Part, GeometryError> {
    let domain = surface.domain_u();
    if west.is_none() && east.is_none() {
        let u = [*original.start(), *original.end()]
            .into_iter()
            .filter(|&u| *domain.start() < u && u < *domain.end())
            .collect::<Vec<_>>();
        let [_, v] = surface.sampled_kink_parameters(tolerance.angular())?;
        let brep = Brep::try_surface_grid(&surface, &u, &v, tolerance)?;
        let mut sides = [Vec::new(), Vec::new()];
        for face in brep.faces() {
            let u = face.surface().domain_u();
            for trim in face.loops()[0].trims() {
                if let Some(edge) = trim.edge() {
                    if trim.iso() == SurfaceIso::West && u.start() == domain.start() {
                        sides[0].push(edge);
                    } else if trim.iso() == SurfaceIso::East && u.end() == domain.end() {
                        sides[1].push(edge);
                    }
                }
            }
        }
        return Ok(Part { brep, sides });
    }
    let west = match west {
        Some(cut) => cut,
        None => Cut::at_u(
            &surface,
            *domain.start(),
            surface.isocurve_v(*domain.start())?,
        )?,
    };
    let east = match east {
        Some(cut) => cut,
        None => Cut::at_u(&surface, *domain.end(), surface.isocurve_v(*domain.end())?)?,
    };
    trimmed_face(surface, west, east, tolerance)
}

fn trimmed_face(
    surface: NurbsSurface,
    west: Cut,
    east: Cut,
    tolerance: Tolerance,
) -> Result<Part, GeometryError> {
    let w = west.uv.domain();
    let e = east.uv.domain();
    let uv = [
        west.uv.evaluate(*w.start())?,
        east.uv.evaluate(*e.start())?,
        east.uv.evaluate(*e.end())?,
        west.uv.evaluate(*w.end())?,
    ];
    let south = surface
        .isocurve_u(uv[0].y())?
        .try_trimmed(uv[0].x()..=uv[1].x())?;
    let north = surface
        .isocurve_u(uv[3].y())?
        .try_trimmed(uv[3].x()..=uv[2].x())?;
    let u = surface.domain_u();
    let side_iso = |cut: &Cut| {
        let x = cut.uv.control_points()[0].point().x();
        if cut.uv.control_points().iter().all(|c| c.point().x() == x) {
            if x == *u.start() {
                SurfaceIso::West
            } else if x == *u.end() {
                SurfaceIso::East
            } else {
                SurfaceIso::InteriorUConstant
            }
        } else {
            SurfaceIso::NotIso
        }
    };
    let iso = [
        SurfaceIso::South,
        side_iso(&east),
        SurfaceIso::North,
        side_iso(&west),
    ];
    let closed = surface.is_closed_v()? && uv[0].x() == uv[3].x() && uv[1].x() == uv[2].x();
    let curves = [south, east.spatial, north, west.spatial];
    let singular = curves.each_ref().map(|curve| {
        let controls = curve.control_points();
        controls.iter().all(|c| c.point() == controls[0].point())
    });
    let mut roots = [0, 1, 2, 3];
    if closed || singular[3] {
        roots[3] = 0;
    }
    if closed || singular[1] {
        roots[2] = 1;
    }
    let mut map = [0; 4];
    let mut vertices = Vec::new();
    for (i, p) in uv.into_iter().enumerate() {
        if roots[i] == i {
            map[i] = vertices.len();
            vertices.push(BrepVertex::try_new(
                surface.evaluate(p.x(), p.y())?,
                tolerance.absolute(),
            )?);
        } else {
            map[i] = map[roots[i]];
        }
    }
    let mut edges = Vec::new();
    let mut ids = [None; 4];
    let endpoints = [[0, 1], [1, 2], [3, 2], [0, 3]].map(|pair| pair.map(|p| map[p]));
    for (i, curve) in curves.into_iter().enumerate() {
        if singular[i] {
            continue;
        }
        if i == 2 && closed {
            ids[2] = ids[0];
            continue;
        }
        ids[i] = Some(edges.len());
        edges.push(BrepEdge::try_new(
            endpoints[i],
            curve,
            tolerance.absolute(),
        )?);
    }
    let parameter_curves = [
        NurbsCurve2::try_line(uv[0], uv[1])?,
        east.uv,
        NurbsCurve2::try_line(uv[2], uv[3])?,
        west.uv.reversed()?,
    ];
    let trims = parameter_curves
        .into_iter()
        .enumerate()
        .map(|(i, p)| {
            let reversed = i >= 2;
            let mut vertices = endpoints[i];
            if reversed {
                vertices.reverse();
            }
            let trim_type = if singular[i] {
                BrepTrimType::Singular
            } else if closed && i % 2 == 0 {
                BrepTrimType::Seam
            } else {
                BrepTrimType::Boundary
            };
            BrepTrim::try_new(
                vertices,
                ids[i],
                reversed && !singular[i],
                p,
                trim_type,
                iso[i],
                [0.; 2],
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let face = BrepFace::try_new(
        surface,
        false,
        vec![BrepLoop::try_new(BrepLoopType::Outer, trims)?],
    )?;
    let brep = Brep::try_new(vertices, edges, vec![face], tolerance)?;
    let sides = [ids[3], ids[1]].map(|edge| edge.into_iter().collect());
    Ok(Part { brep, sides })
}
