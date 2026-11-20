//! Profile transport in the moving frame defined by both rails.
use super::{Section, unsupported};
use crate::native::{
    Error, change_curve_degree, change_surface_degree, insert_curve_knot, insert_surface_knot,
    knot_refinement::{clamp_curve, solve_uniform_system},
    ruled_surface,
};
use viboceros_geometry::{
    GeometryError, NurbsCurve, NurbsSurface, Point3, SectionWeightScale, SurfaceKnotDirection,
    UniformInterpolationSystem, Vector3, prepare_nurbs_sections_with_operations,
};

struct Frame {
    origin: Point3,
    axes: [Vector3; 3],
    width: f64,
}

impl Frame {
    fn new(a: Point3, b: Point3, tangent: Vector3) -> Result<Self, Error> {
        let chord = a.vector_to(b)?;
        let x = chord.normalized_nonzero()?.as_vector();
        let y = tangent.cross(x)?.normalized_nonzero()?.as_vector();
        let z = x.cross(y)?;
        Ok(Self {
            origin: a,
            axes: [x, y, z],
            width: chord.length()?,
        })
    }

    fn at(rails: [&NurbsCurve; 2], station: f64) -> Result<Self, Error> {
        let [a, b] = rails;
        Self::new(
            a.evaluate(station)?,
            b.evaluate(station)?,
            tangent(rails, station)?,
        )
    }

    fn coordinates(&self, p: Point3, maintain_height: bool) -> Result<[f64; 3], Error> {
        let offset = self.origin.vector_to(p)?;
        let [x, y, z] = self.axes;
        Ok([
            offset.dot(x)? / self.width,
            offset.dot(y)? / if maintain_height { 1. } else { self.width },
            offset.dot(z)? / self.width,
        ])
    }

    fn point(&self, [x, y, z]: [f64; 3], maintain_height: bool) -> Result<Point3, Error> {
        let [across, height, along] = self.axes;
        let u = across.scaled(x * self.width)?;
        let v = height.scaled(y * if maintain_height { 1. } else { self.width })?;
        let w = along.scaled(z * self.width)?;
        Ok(self.origin.translated(add(add(u, v)?, w)?)?)
    }
}

fn add(a: Vector3, b: Vector3) -> Result<Vector3, GeometryError> {
    Vector3::try_new(a.x() + b.x(), a.y() + b.y(), a.z() + b.z())
}

fn tangent(rails: [&NurbsCurve; 2], station: f64) -> Result<Vector3, Error> {
    Ok(add(
        rails[0].derivative_at(station)?,
        rails[1].derivative_at(station)?,
    )?)
}

fn greville(knots: &[f64], degree: usize) -> Vec<f64> {
    knots[1..knots.len() - 1]
        .windows(degree)
        .map(|window| window.iter().sum::<f64>() / degree as f64)
        .collect()
}

pub(super) fn sweep(
    rails: [&NurbsCurve; 2],
    rows: &[Section],
    maintain_height: bool,
) -> Result<NurbsSurface, Error> {
    // The native ruled tensor puts the two exact rail columns in a shared
    // longitudinal basis (its V direction), with no changes to their charts.
    let strip = ruled_surface(rails[0], rails[1])?;
    let profiles = rows.iter().map(|row| row.curve.clone()).collect::<Vec<_>>();
    let profiles = prepare_nurbs_sections_with_operations(
        &profiles,
        1024,
        GeometryError::InvalidControlNet {
            context: "Sweep2 section basis exceeds 1024 controls",
        },
        SectionWeightScale::Preserve,
        &mut clamp_curve,
        &mut |curve, degree| change_curve_degree(curve, degree, false),
        &mut insert_curve_knot,
    )?;
    let profile = &profiles[0];
    let count_v = profile.control_points().len();
    let stations = rows.iter().map(|row| row.stations[0]).collect::<Vec<_>>();
    let prepared = stations
        .iter()
        .zip(&profiles)
        .map(|(&station, curve)| {
            let controls = curve.control_points();
            if controls.iter().all(|c| c.point() == controls[0].point()) {
                return Ok(vec![[0.; 3]; count_v]);
            }
            let frame = Frame::at(rails, station)?;
            controls
                .iter()
                .map(|p| frame.coordinates(p.point(), maintain_height))
                .collect::<Result<Vec<_>, Error>>()
        })
        .collect::<Result<Vec<_>, Error>>()?;

    let interpolate = maintain_height || rows.len() > 1;
    let strip = if interpolate {
        blend_basis(strip, &stations)?
    } else {
        strip
    };
    let degree = strip.degree_v();
    let knots = strip.knots_v();
    let count_u = strip.control_point_count_v();
    if count_u.saturating_mul(count_v) > 65536 {
        return Err(unsupported("transport control-net resource limit"));
    }
    let mut sites = greville(knots, degree);
    let ends = |i| {
        [
            strip.control_point(0, i).unwrap().point(),
            strip.control_point(1, i).unwrap().point(),
        ]
    };
    let mut targets = Vec::with_capacity(count_u * count_v * 4);
    if interpolate {
        pin_sections(&mut sites, &stations)?;
        for &site in &sites {
            if let Ok(i) = stations.binary_search_by(|s| s.total_cmp(&site)) {
                for p in profiles[i].control_points() {
                    targets.extend(p.point().to_array());
                    targets.push(1.);
                }
                continue;
            }
            let frame = Frame::at(rails, site)?;
            let hi = stations
                .partition_point(|s| *s < site)
                .min(stations.len() - 1);
            let lo = hi.saturating_sub(1);
            let blend = if lo == hi {
                0.
            } else {
                let t = (site - stations[lo]) / (stations[hi] - stations[lo]);
                t * t * (3. - 2. * t)
            };
            for (a, b) in prepared[lo].iter().zip(&prepared[hi]) {
                let p = frame.point(
                    std::array::from_fn(|i| a[i] + blend * (b[i] - a[i])),
                    maintain_height,
                )?;
                targets.extend(p.to_array());
                targets.push(1.);
            }
        }
        targets = solve_uniform_system(&UniformInterpolationSystem {
            degree,
            knots,
            parameters: &sites,
            periodic: false,
            columns: 4 * count_v,
            targets: &targets,
        })?;
    } else {
        // Uniform scaling on the retained rail control polygons gives exact
        // transverse chords and keeps the native longitudinal degree.
        for (i, &site) in sites.iter().enumerate() {
            let [a, b] = ends(i);
            let anchor = (site == stations[0]).then_some(profile.control_points());
            let frame = (anchor.is_none() && a != b)
                .then(|| Frame::new(a, b, tangent(rails, site)?))
                .transpose()?;
            for (v, local) in prepared[0].iter().enumerate() {
                let p = match (anchor, &frame) {
                    (Some(controls), _) => controls[v].point(),
                    (_, Some(frame)) => frame.point(*local, false)?,
                    _ => a,
                };
                targets.extend(p.to_array());
                targets.push(1.);
            }
        }
    }
    let mut controls = Vec::with_capacity(count_u * count_v);
    for v in 0..count_v {
        for u in 0..count_u {
            let offset = 4 * (u * count_v + v);
            let p = match v {
                0 => ends(u)[0],
                index if index + 1 == count_v => ends(u)[1],
                _ => Point3::try_from([targets[offset], targets[offset + 1], targets[offset + 2]])?,
            };
            controls.push(p);
        }
    }
    Ok(NurbsSurface::try_new(
        degree,
        profile.degree(),
        count_u,
        count_v,
        controls,
        knots.to_vec(),
        profile.knots().to_vec(),
    )?)
}

fn blend_basis(mut strip: NurbsSurface, stations: &[f64]) -> Result<NurbsSurface, Error> {
    if strip.degree_v() < 3 {
        strip = change_surface_degree(&strip, 1, 3, false)?;
    }
    if stations.len() > 2 {
        // Samples at Greville sites interpolate the smooth blend on either
        // side of each picked section. Keep the exact original rail basis.
        for t in stations
            .windows(2)
            .map(|s| (s[0] + s[1]) * 0.5)
            .chain(stations[1..stations.len() - 1].iter().copied())
        {
            if !strip.knots_v().contains(&t) {
                strip = insert_surface_knot(&strip, SurfaceKnotDirection::V, t, 1)?;
            }
        }
    }
    Ok(strip)
}

fn pin_sections(sites: &mut [f64], stations: &[f64]) -> Result<(), Error> {
    for &t in stations.iter().filter(|&&t| t > 0. && t < 1.) {
        let hi = sites.partition_point(|s| *s < t);
        let lo = hi - 1;
        let i = if lo == 0 || (hi + 1 < sites.len() && sites[hi] - t <= t - sites[lo]) {
            hi
        } else {
            lo
        };
        sites[i] = t;
    }
    if sites.windows(2).any(|s| s[0] >= s[1])
        || stations
            .iter()
            .any(|t| sites.binary_search_by(|s| s.total_cmp(t)).is_err())
    {
        return Err(unsupported(
            "could not place distinct profile interpolation sites",
        ));
    }
    Ok(())
}
