//! Assemble open source pieces without modifying their document objects.
use super::*;
mod filled;
mod linear;
mod walks;
use viboceros_geometry::{Curve3, CurveJoinOptions, CurveJoinStyle};

pub(super) struct Prepared {
    pub profiles: Vec<NurbsCurve>,
    pub sources: Vec<Vec<ObjectId>>,
    recovered: Vec<bool>,
    paths: Vec<NurbsCurve>,
    source_inputs: Option<(Vec<ObjectId>, Vec<NurbsCurve>)>,
}
#[derive(Default)]
struct Budget {
    controls: usize,
    work: usize,
}
impl Budget {
    fn work(&mut self, n: usize) -> Result<(), GeometryError> {
        self.work = self.work.checked_add(n).filter(|&n| n <= 1_000_000).ok_or(
            GeometryError::CurveJoinLimit {
                resource: "planar boundary assembly",
                maximum: 1_000_000,
            },
        )?;
        Ok(())
    }
    fn output(&mut self, n: usize) -> Result<(), GeometryError> {
        self.controls = self
            .controls
            .checked_add(n)
            .filter(|&n| n <= 100_000)
            .ok_or(GeometryError::CurveJoinLimit {
                resource: "planar boundary controls",
                maximum: 100_000,
            })?;
        Ok(())
    }
}
pub(super) fn prepare(
    curves: Vec<(ObjectId, NurbsCurve)>,
    tolerance: Tolerance,
    with_sources: bool,
) -> Result<Prepared, CommandError> {
    let (ids, originals): (Vec<_>, Vec<_>) = curves.into_iter().unzip();
    originals.iter().try_fold(0usize, |n, c| {
        n.checked_add(c.control_points().len())
            .filter(|&n| n <= 100_000)
            .ok_or(GeometryError::CurveJoinLimit {
                resource: "planar boundary controls",
                maximum: 100_000,
            })
    })?;
    let curves = originals
        .iter()
        .cloned()
        .map(Curve3::NurbsCurve)
        .collect::<Vec<_>>();
    let joined = viboceros_geometry::join_curves(
        &curves,
        CurveJoinOptions {
            tolerance: tolerance.absolute().next_down(),
            preserve_direction: false,
            style: CurveJoinStyle::Batch,
        },
        Tolerance::NUMERICAL_VALIDATION,
    )?;
    let mut profiles = Vec::new();
    let mut guaranteed = Vec::new();
    let mut recovered = Vec::new();
    let mut paths = Vec::new();
    let mut budget = Budget::default();
    for chain in joined {
        let members = if with_sources {
            chain.source_indices().to_vec()
        } else {
            Vec::new()
        };
        let curve = chain.into_curve();
        let (curve, _) =
            curve.close(tolerance.absolute(), false, Tolerance::NUMERICAL_VALIDATION)?;
        let closed = curve.as_ref().is_closed()?;
        let mut network = Vec::new();
        let mut contours = match &curve {
            Curve3::Polyline(p) => linear::contours(p, tolerance, &mut budget, &mut network)?,
            Curve3::PolyCurve(p) if !closed => walks::contours(p, &mut budget)?,
            _ => {
                let c = curve.as_ref().to_nurbs()?;
                budget.output(c.control_points().len())?;
                vec![c]
            }
        };
        if !closed
            && contours.is_empty()
            && let Some(plane) = curve.as_ref().supporting_plane_frame(tolerance)?
        {
            contours = viboceros_geometry::planar_curve_network_boundaries(
                &[curve.as_ref().to_nurbs()?],
                plane,
                tolerance,
            )?;
            for c in &contours {
                budget.output(c.control_points().len())?;
            }
        }
        for contour in contours {
            if contour.is_closed()? && contour.is_planar(tolerance)? {
                if profiles.len() == 128 {
                    return Err(CommandError::Usage(
                        "PlanarSrf supports at most 128 closed planar boundaries",
                    ));
                }
                let complete = match &curve {
                    Curve3::Polyline(p) => {
                        p.is_closed()
                            && p.vertices().len() == contour.control_points().len()
                            && p.vertices()
                                .iter()
                                .zip(contour.control_points())
                                .all(|(a, b)| *a == b.point())
                    }
                    _ => closed,
                };
                profiles.push(contour);
                recovered.push(!closed);
                if with_sources {
                    guaranteed.push(if complete {
                        members.clone()
                    } else {
                        Vec::new()
                    });
                }
            }
        }
        paths.push(curve);
        paths.extend(network);
    }
    let paths = if recovered.iter().any(|r| *r) {
        paths
            .into_iter()
            .map(|p| p.as_ref().to_nurbs())
            .collect::<Result<Vec<_>, _>>()?
    } else {
        Vec::new()
    };
    let sources = if with_sources {
        viboceros_geometry::curve_overlap_sources(&profiles, &originals, tolerance)?
            .into_iter()
            .zip(guaranteed)
            .map(|(members, guaranteed)| {
                members
                    .into_iter()
                    .chain(guaranteed)
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .map(|i| ids[i])
                    .collect()
            })
            .collect()
    } else {
        Vec::new()
    };
    Ok(Prepared {
        profiles,
        sources,
        recovered,
        paths,
        source_inputs: with_sources.then_some((ids, originals)),
    })
}

impl Prepared {
    pub(super) fn stage(
        &self,
        tolerance: Tolerance,
    ) -> Result<(Vec<Brep>, Vec<ObjectId>), CommandError> {
        let recovered = self.recovered.iter().any(|r| *r);
        let (faces, used) = if recovered {
            filled::stage(self, tolerance)
        } else {
            stage_profiles(self.profiles.clone(), tolerance, false)
        }?;
        let mut consumed = used
            .into_iter()
            .flat_map(|i| self.sources.get(i).into_iter().flatten().copied())
            .collect::<BTreeSet<_>>();
        if recovered && let Some((ids, originals)) = &self.source_inputs {
            let boundaries = faces
                .iter()
                .flat_map(|b| b.edges().iter().map(|e| e.curve().clone()))
                .collect::<Vec<_>>();
            for members in
                viboceros_geometry::curve_overlap_sources(&boundaries, originals, tolerance)?
            {
                consumed.extend(members.into_iter().map(|i| ids[i]));
            }
        }
        Ok((faces, consumed.into_iter().collect()))
    }
}
