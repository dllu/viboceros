//! Sweep2 rail refitting in the mean arc-length chart of the profile contacts.
use super::{
    Section,
    transport::{ProfileBlend, greville, pin_sections},
    unsupported,
};
use crate::native::{Error, knot_refinement::solve_uniform_system};
use std::collections::BTreeMap;
use viboceros_geometry::{
    ArcLengthSampler, CurveRef, NurbsCurve, NurbsSurface, Point3, Tolerance,
    UniformInterpolationSystem, Vector3,
};

const DEGREE: usize = 3;
const MAX_RAIL_CONTROLS: usize = 1024;

struct Contact {
    source: [f64; 2],
    distances: [f64; 2],
    station: f64,
}

pub(super) struct Clock<'a> {
    samplers: [ArcLengthSampler<'a>; 2],
    contacts: Vec<Contact>,
    length: f64,
}

impl<'a> Clock<'a> {
    fn new(
        rails: [&'a NurbsCurve; 2],
        sources: &[[f64; 2]],
        tolerance: Tolerance,
    ) -> Result<Self, Error> {
        let mut samplers = [
            ArcLengthSampler::try_new(CurveRef::NurbsCurve(rails[0]), tolerance)?,
            ArcLengthSampler::try_new(CurveRef::NurbsCurve(rails[1]), tolerance)?,
        ];
        for sampler in &mut samplers {
            sampler.prepare_budgeted_repeated_sampling(32)?;
        }
        let length = samplers[0]
            .total_length()
            .midpoint(samplers[1].total_length());
        let contacts = sources
            .iter()
            .map(|&source| {
                let distances = [
                    samplers[0].distance_at_parameter(source[0])?,
                    samplers[1].distance_at_parameter(source[1])?,
                ];
                let station = distances[0].midpoint(distances[1]) / length;
                Ok(Contact {
                    source,
                    distances,
                    station,
                })
            })
            .collect::<Result<Vec<_>, Error>>()?;
        if contacts.windows(2).any(|c| {
            c[0].station >= c[1].station || (0..2).any(|i| c[0].distances[i] >= c[1].distances[i])
        }) {
            return Err(unsupported(
                "refit requires distinct arc-length contacts on both rails",
            ));
        }
        Ok(Self {
            samplers,
            contacts,
            length,
        })
    }

    fn locations(&self, station: f64) -> Result<[f64; 2], Error> {
        let left = match self
            .contacts
            .binary_search_by(|c| c.station.total_cmp(&station))
        {
            Ok(i) => return Ok(self.contacts[i].source),
            Err(i) => i - 1,
        };
        let (a, b) = (&self.contacts[left], &self.contacts[left + 1]);
        let t = (station - a.station) / (b.station - a.station);
        let interpolate = |i| {
            let a: f64 = a.distances[i];
            a.mul_add(1. - t, b.distances[i] * t)
        };
        Ok([
            self.samplers[0].parameter_at_distance(interpolate(0))?,
            self.samplers[1].parameter_at_distance(interpolate(1))?,
        ])
    }

    fn station_on_rail(&self, rail: usize, parameter: f64) -> Result<f64, Error> {
        let hi = self
            .contacts
            .partition_point(|contact| contact.source[rail] < parameter);
        let (a, b) = (&self.contacts[hi - 1], &self.contacts[hi]);
        let distance = self.samplers[rail].distance_at_parameter(parameter)?;
        let t = (distance - a.distances[rail]) / (b.distances[rail] - a.distances[rail]);
        Ok(a.station + t * (b.station - a.station))
    }

    pub(super) fn certify_rail(
        &self,
        rail: usize,
        source: &NurbsCurve,
        fit: &NurbsCurve,
        limit: f64,
    ) -> Result<bool, Error> {
        let domain = fit.domain();
        let knots = &fit.knots()[fit.degree()..=fit.control_points().len()];
        let pairs = knots
            .chunk_by(|a, b| a == b)
            .map(|knot| {
                let t = (knot[0] - domain.start()) / (domain.end() - domain.start());
                Ok([self.locations(t)?[rail], t])
            })
            .collect::<Result<Vec<_>, Error>>()?;
        Ok(source
            .curve_image_deviation_bound(fit, &pairs, limit)?
            .is_some())
    }
}

#[derive(Clone, Copy)]
struct Sample {
    points: [Point3; 2],
    tangents: [Option<Vector3>; 2],
}

struct RailSamples<'a> {
    rails: [&'a NurbsCurve; 2],
    clock: Clock<'a>,
    cache: BTreeMap<u64, Sample>,
}

impl RailSamples<'_> {
    fn at(&mut self, t: f64) -> Result<Sample, Error> {
        let key = t.to_bits();
        if let Some(sample) = self.cache.get(&key) {
            return Ok(*sample);
        }
        let [a, b] = self.clock.locations(t)?;
        let [first, second] = [
            self.rails[0].evaluate_with_derivative(a)?,
            self.rails[1].evaluate_with_derivative(b)?,
        ];
        let sample = Sample {
            points: [first.0, second.0],
            tangents: [first.1, second.1]
                .map(|d| d.normalized_nonzero().ok().map(|u| u.as_vector())),
        };
        self.cache.insert(key, sample);
        Ok(sample)
    }

    fn interpolate(&mut self, knots: &[f64], sites: &[f64]) -> Result<[NurbsCurve; 2], Error> {
        let mut targets = Vec::with_capacity(8 * sites.len());
        for &site in sites {
            for p in self.at(site)?.points {
                targets.extend(p.to_array());
                targets.push(1.);
            }
        }
        let result = solve_uniform_system(&UniformInterpolationSystem {
            degree: DEGREE,
            knots,
            parameters: sites,
            periodic: false,
            columns: 8,
            targets: &targets,
        })?;
        let curve = |rail: usize| -> Result<NurbsCurve, Error> {
            let controls = result
                .chunks_exact(8)
                .map(|row| Point3::try_from([row[4 * rail], row[4 * rail + 1], row[4 * rail + 2]]))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(NurbsCurve::try_new(DEGREE, controls, knots.to_vec())?)
        };
        Ok([curve(0)?, curve(1)?])
    }

    fn span_needs_refit(
        &mut self,
        a: f64,
        b: f64,
        fitted: &[NurbsCurve; 2],
        limit: f64,
        use_pointwise_error: bool,
    ) -> Result<bool, Error> {
        for fraction in [0.125, 0.25, 0.5, 0.75, 0.875] {
            let t = a + fraction * (b - a);
            let sample = self.at(t)?;
            for (i, rail) in fitted.iter().enumerate() {
                let delta = sample.points[i].vector_to(rail.evaluate(t)?)?;
                // At a profile contact the two arc clocks may change relative
                // speed. The fit can smooth those clocks along the rail images.
                let error = match sample.tangents[i] {
                    Some(unit) if !use_pointwise_error => delta.cross(unit)?.length()?,
                    _ => delta.length()?,
                };
                if error > limit {
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }
}

pub(super) struct FittedSweep<'a> {
    pub(super) surface: NurbsSurface,
    pub(super) stations: Vec<f64>,
    pub(super) clock: Clock<'a>,
}

pub(super) fn sweep<'a>(
    rails: [&'a NurbsCurve; 2],
    rows: &[Section],
    paired_stations: &[[f64; 2]],
    maintain_height: bool,
    closed_profiles: bool,
    tolerance: Tolerance,
) -> Result<FittedSweep<'a>, Error> {
    let clock = Clock::new(rails, paired_stations, tolerance)?;
    let all_stations = clock.contacts.iter().map(|c| c.station).collect::<Vec<_>>();
    let stations = rows
        .iter()
        .map(|row| {
            let i = paired_stations
                .binary_search_by(|pair| pair[0].total_cmp(&row.stations[0]))
                .expect("profile station retained by placement");
            all_stations[i]
        })
        .collect::<Vec<_>>();
    let mut spans = BTreeMap::from([(0_f64.to_bits(), 4), (1_f64.to_bits(), 4)]);
    for s in all_stations.windows(3) {
        // Put a Greville site at each paired contact using three symmetric
        // simple knots; refinement can add more support away from the contact.
        let half = (s[1] - s[0]).min(s[2] - s[1]) * 0.5;
        for t in [s[1] - half, s[1], s[1] + half] {
            spans.insert(t.to_bits(), 1);
        }
    }
    let mut has_kinks = false;
    for (i, rail) in rails.iter().enumerate() {
        for kink in rail.full_multiplicity_kink_parameters(tolerance)? {
            let t = clock.station_on_rail(i, kink)?;
            spans.insert(t.to_bits(), 3);
            has_kinks = true;
        }
    }
    let blend = ProfileBlend::new(
        rails,
        rows,
        stations.clone(),
        maintain_height,
        closed_profiles,
    )?;
    let mut samples = RailSamples {
        rails,
        clock,
        cache: BTreeMap::new(),
    };
    loop {
        let knots = spans
            .iter()
            .flat_map(|(&bits, &m)| std::iter::repeat_n(f64::from_bits(bits), m))
            .collect::<Vec<_>>();
        if knots.len() - DEGREE - 1 > MAX_RAIL_CONTROLS {
            return Err(unsupported("rail refit control limit"));
        }
        let mut sites = greville(&knots, DEGREE);
        pin_sections(&mut sites, &all_stations)?;
        let fitted = samples.interpolate(&knots, &sites)?;
        let mut add = Vec::new();
        let breaks = spans
            .keys()
            .copied()
            .map(f64::from_bits)
            .collect::<Vec<_>>();
        for interval in breaks.windows(2) {
            let [a, b] = [interval[0], interval[1]];
            if samples.span_needs_refit(
                a,
                b,
                &fitted,
                tolerance.absolute() * 0.25,
                all_stations.len() == 2 && !has_kinks,
            )? {
                let mid = a.midpoint(b);
                if mid <= a || mid >= b {
                    return Err(unsupported("could not refine a rail interval"));
                }
                add.push((mid.to_bits(), 2));
            }
        }
        if add.is_empty() {
            let controls = blend.interpolate(rails, DEGREE, &knots, &sites, |t| {
                samples.clock.locations(t)
            })?;
            let surface = blend.tensor(DEGREE, &knots, &controls, None, |i| {
                [
                    fitted[0].control_points()[i].point(),
                    fitted[1].control_points()[i].point(),
                ]
            })?;
            let surface = surface.try_reparameterized(0. ..=samples.clock.length, 0. ..=1.)?;
            return Ok(FittedSweep {
                surface,
                stations,
                clock: samples.clock,
            });
        }
        spans.extend(add);
    }
}
