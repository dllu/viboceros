//! Octagonal control cages for subdivided pipe walls and caps.
use super::*;
use std::f64::consts::FRAC_1_SQRT_2;
use viboceros_geometry::SubD;

#[cfg(test)]
mod tests;

const SIDES: u32 = 8;
// Inverse of the periodic cubic limit mask on a regular octagon.
const RADIUS_SCALE: Real = 3. / (2. + FRAC_1_SQRT_2);
const OCTAGON: [[Real; 2]; SIDES as usize] = [
    [FRAC_1_SQRT_2, FRAC_1_SQRT_2],
    [1., 0.],
    [FRAC_1_SQRT_2, -FRAC_1_SQRT_2],
    [0., -1.],
    [-FRAC_1_SQRT_2, -FRAC_1_SQRT_2],
    [-1., 0.],
    [-FRAC_1_SQRT_2, FRAC_1_SQRT_2],
    [0., 1.],
];

#[derive(Default)]
pub struct ControlCage {
    points: Vec<Point3>,
    faces: Vec<Vec<u32>>,
    crease_rims: Vec<u32>,
}

impl ControlCage {
    pub fn segments(&self) -> impl Iterator<Item = (Point3, Point3)> + '_ {
        let mut seen = std::collections::HashSet::new();
        self.faces
            .iter()
            .flat_map(|face| face.iter().zip(face.iter().cycle().skip(1)))
            .filter_map(move |(&a, &b)| {
                seen.insert((a.min(b), a.max(b)))
                    .then(|| (self.points[a as usize], self.points[b as usize]))
            })
    }

    fn into_geometry(self) -> Result<Geometry, GeometryError> {
        let subd = SubD::try_from_polygons(self.points, self.faces)?;
        if self.crease_rims.is_empty() {
            return Ok(Geometry::SubD(subd));
        }
        let tags = subd
            .edges()
            .iter()
            .enumerate()
            .filter_map(|(index, edge)| {
                self.crease_rims
                    .iter()
                    .any(|&rim| edge.iter().all(|v| (rim..rim + SIDES).contains(v)))
                    .then_some((index, 2))
            })
            .collect::<Vec<_>>();
        Ok(Geometry::SubD(subd.try_with_edge_tags(&tags)?))
    }

    fn ring(&mut self, frame: Frame3, radius: Real, reverse: bool) -> Result<u32, GeometryError> {
        let first = self.points.len() as u32;
        for [x, y] in OCTAGON {
            let y = if reverse { -y } else { y };
            self.points
                .push(frame.point_at([x * radius, y * radius, 0.])?);
        }
        Ok(first)
    }

    fn wall(
        &mut self,
        axis: &StraightRail,
        profile: &Profile,
        blend: SweepBlend,
        reverse: bool,
    ) -> Result<[u32; 2], GeometryError> {
        let rings = control_stations(axis, profile, blend)
            .into_iter()
            .map(|[distance, radius]| {
                self.ring(axis.frame_at(distance)?, radius * RADIUS_SCALE, reverse)
            })
            .collect::<Result<Vec<_>, _>>()?;
        for side in 0..SIDES {
            let next = (side + 1) % SIDES;
            for pair in rings.windows(2) {
                let [a, b] = [pair[0], pair[1]];
                self.faces
                    .push(vec![a + side, b + side, b + next, a + next]);
            }
        }
        Ok([rings[0], *rings.last().unwrap()])
    }

    fn flat_cap(&mut self, rim: u32, center: Point3, start: bool) {
        let pole = self.points.len() as u32;
        self.points.push(center);
        self.crease_rims.push(rim);
        for side in 0..SIDES {
            let mut vertices = rim_pair(rim, side);
            if !start {
                vertices.reverse();
            }
            self.faces.push(vec![vertices[0], vertices[1], pole]);
        }
    }

    fn annular_cap(&mut self, outer: u32, inner: u32, start: bool) {
        self.crease_rims.extend([outer, inner]);
        for side in 0..SIDES {
            let [a, b] = rim_pair(outer, side);
            // The inner rim runs in reverse, with its seam across the local X axis.
            let c = inner + (SIDES + 1 - side) % SIDES;
            let d = inner + (SIDES + 2 - side) % SIDES;
            self.faces.push(if start {
                vec![a, b, c, d]
            } else {
                vec![b, a, d, c]
            });
        }
    }

    fn round_cap(
        &mut self,
        rim: u32,
        frame: Frame3,
        radius: Real,
        start: bool,
    ) -> Result<(), GeometryError> {
        let first = self.points.len() as u32;
        let sign = if start { -1. } else { 1. };
        for (side, [x, y]) in OCTAGON.into_iter().enumerate() {
            // Radial control coefficients of the spherical end patch. The
            // axial coordinate follows the larger radial coordinate.
            let scale = if side % 2 == 0 {
                0.8163263529078808
            } else {
                0.7657193460172731
            };
            let r = radius * scale;
            self.points
                .push(frame.point_at([x * r, y * r, sign * x.abs().max(y.abs()) * r])?);
        }
        let pole = self.points.len() as u32;
        self.points
            .push(frame.point_at([0., 0., sign * radius * RADIUS_SCALE])?);
        for side in 0..SIDES {
            let [a, b] = rim_pair(rim, side);
            let [c, d] = rim_pair(first, side);
            self.faces.push(if start {
                vec![a, b, d, c]
            } else {
                vec![b, a, c, d]
            });
        }
        for side in (1..SIDES).step_by(2) {
            let [a, b] = rim_pair(first, side);
            let c = first + (side + 2) % SIDES;
            self.faces.push(if start {
                vec![a, b, c, pole]
            } else {
                vec![b, a, pole, c]
            });
        }
        Ok(())
    }
}

fn rim_pair(rim: u32, side: u32) -> [u32; 2] {
    [rim + side, rim + (side + 1) % SIDES]
}

struct StraightRail {
    frame: Frame3,
    height: Real,
}

impl StraightRail {
    fn from_curve(rail: CurveRef<'_>, tolerance: Tolerance) -> Result<Self, CommandError> {
        if !matches!(rail, CurveRef::Line(_)) && !rail.to_nurbs()?.is_linear(tolerance)? {
            return Err(CommandError::Usage(
                "Pipe SubD output requires a straight rail",
            ));
        }
        let start = rail.start_point()?;
        let path = start.vector_to(rail.end_point()?)?;
        Ok(Self {
            frame: Frame3::try_from_normal(start, path, tolerance)?,
            height: path.length()?,
        })
    }

    fn frame_at(&self, distance: Real) -> Result<Frame3, GeometryError> {
        let origin = self.frame.point_at([0., 0., distance])?;
        Ok(self.frame.with_origin(origin))
    }

    fn distance_at(&self, rail: CurveRef<'_>, fraction: Real) -> Result<Real, GeometryError> {
        let point = rail.evaluate(rail.parameter_at(fraction)?)?;
        self.frame
            .origin()
            .vector_to(point)?
            .dot(self.frame.z_axis().as_vector())
    }
}

fn control_stations(axis: &StraightRail, profile: &Profile, blend: SweepBlend) -> Vec<[Real; 2]> {
    let [first, last] = profile.endpoints;
    let constant = first == last && profile.stations.iter().all(|&(_, radius)| radius == first);
    if constant || blend == SweepBlend::Global {
        return vec![[0., first], [axis.height, last]];
    }
    let stations = [(0., first)]
        .into_iter()
        .chain(profile.stations.iter().copied())
        .chain([(axis.height, last)])
        .collect::<Vec<_>>();
    let mut knots = vec![0.; 4];
    for pair in stations.windows(3) {
        let at = pair[1].0;
        // Keep a Greville sample at each radius station by placing neighboring
        // knots symmetrically within the available spans.
        let reach = (at - knots.last().unwrap()).min(pair[2].0 - at) * 0.5;
        knots.extend([at - reach, at, at + reach]);
    }
    knots.extend([axis.height; 4]);
    let mut interval = 0;
    let mut samples = knots[1..knots.len() - 1]
        .windows(3)
        .map(|k| {
            let distance = (k[0] + k[1] + k[2]) / 3.;
            while interval + 2 < stations.len() && distance > stations[interval + 1].0 {
                interval += 1;
            }
            let [(a, r0), (b, r1)] = [stations[interval], stations[interval + 1]];
            let f = (distance - a) / (b - a);
            let t = f * f * (3. - 2. * f);
            [distance, r0 + (r1 - r0) * t]
        })
        .collect::<Vec<_>>();
    interpolate_control_stations(&mut samples);
    samples
}

// Invert the cubic vertex mask along each radial column while retaining the
// exact end samples. Both the distance and radius share the same factorization.
fn interpolate_control_stations(samples: &mut [[Real; 2]]) {
    let end = samples.len() - 1;
    let mut upper = vec![0.; end];
    for i in 1..end {
        let divisor = 4. - upper[i - 1];
        upper[i] = 1. / divisor;
        for axis in 0..2 {
            let last = if i + 1 == end { samples[end][axis] } else { 0. };
            samples[i][axis] = (6. * samples[i][axis] - samples[i - 1][axis] - last) / divisor;
        }
    }
    for i in (1..end - 1).rev() {
        for axis in 0..2 {
            samples[i][axis] -= upper[i] * samples[i + 1][axis];
        }
    }
}

pub(super) fn cages(
    rail: CurveRef<'_>,
    a: &Profile,
    b: Option<&Profile>,
    options: PipeOptions,
    tolerance: Tolerance,
) -> Result<Vec<ControlCage>, CommandError> {
    if options.blend == SweepBlend::Global && !a.stations.is_empty() {
        return Err(CommandError::Usage(
            "Pipe SubD output with intermediate radii requires Local shape blending",
        ));
    }
    let axis = StraightRail::from_curve(rail, tolerance)?;
    let at_distance = |p: &Profile| -> Result<Profile, GeometryError> {
        let stations = p
            .stations
            .iter()
            .map(|&(f, r)| Ok((axis.distance_at(rail, f)?, r)))
            .collect::<Result<Vec<_>, GeometryError>>()?;
        Ok(Profile::new(p.endpoints, stations))
    };
    let mut cage = ControlCage::default();
    if let Some(b) = b {
        let outer = at_distance(&a.combine(b, Real::max))?;
        let inner = at_distance(&a.combine(b, Real::min))?;
        let outer = cage.wall(&axis, &outer, options.blend, false)?;
        if options.cap == PipeCap::None {
            let mut other = ControlCage::default();
            other.wall(&axis, &inner, options.blend, true)?;
            return Ok(vec![cage, other]);
        }
        let inner = cage.wall(&axis, &inner, options.blend, true)?;
        for i in 0..2 {
            cage.annular_cap(outer[i], inner[i], i == 0);
        }
    } else {
        let frames = [axis.frame, axis.frame_at(axis.height)?];
        let rims = cage.wall(&axis, &at_distance(a)?, options.blend, false)?;
        for (i, (rim, frame)) in rims.into_iter().zip(frames).enumerate() {
            match options.cap {
                PipeCap::None => {}
                PipeCap::Flat => cage.flat_cap(rim, frame.origin(), i == 0),
                PipeCap::Round => cage.round_cap(rim, frame, a.endpoints[i], i == 0)?,
            }
        }
    }
    Ok(vec![cage])
}

pub(super) fn build_walls(
    rail: CurveRef<'_>,
    first: &Profile,
    second: Option<&Profile>,
    options: PipeOptions,
    tolerance: Tolerance,
) -> Result<Vec<Geometry>, CommandError> {
    cages(rail, first, second, options, tolerance)?
        .into_iter()
        .map(|cage| Ok(cage.into_geometry()?))
        .collect()
}
