//! Live sections with the same provisional radii as the point getters.
use super::*;

pub struct Draft {
    pub sections: Vec<(Frame3, [Real; 2])>,
    pub active: Option<(Frame3, [Real; 2])>,
    pub round_caps: bool,
}

impl Input {
    pub fn draft(
        &self,
        rail: &Rail,
        cursor: Option<Point3>,
        tolerance: Tolerance,
    ) -> Option<Draft> {
        if self.selecting() {
            return None;
        }
        let mut sections = self.sections.clone();
        let mut active = None;
        match self.stage {
            Stage::Radius { fraction, first } => {
                let frame = rail.frame(fraction).ok()?;
                let measured = cursor
                    .and_then(|p| measured_radius(frame, p).ok())
                    .filter(|r| *r > 0.);
                let candidate = measured.unwrap_or_else(|| self.default_size(first));
                let radii = if let Some(first) = first {
                    [first, candidate]
                } else {
                    [
                        candidate,
                        self.fixed_thickness.map_or(0., |t| (candidate + t).max(0.)),
                    ]
                };
                let index = sections.partition_point(|&(f, _)| f < fraction);
                sections.insert(index, (fraction, radii));
                active = Some((frame, radii));
            }
            Stage::Thickness { fraction, first } => {
                let radii = [first, (first + self.settings.thickness).max(0.)];
                let index = sections.partition_point(|&(f, _)| f < fraction);
                sections.insert(index, (fraction, radii));
                active = Some((rail.frame(fraction).ok()?, radii));
            }
            Stage::Station if let Some(point) = cursor => {
                let fraction = rail.fraction_at(point, tolerance).ok()?;
                active = Some((
                    rail.frame(fraction).ok()?,
                    interpolate(&sections, fraction, self.settings.blend),
                ));
            }
            _ => {}
        }
        let first = sections.first()?.1;
        if self.closed {
            sections.push((1., first));
        }
        let sections = rail
            .frames()
            .iter()
            .map(|&(f, frame)| (frame, interpolate(&sections, f, self.settings.blend)))
            .collect();
        Some(Draft {
            sections,
            active,
            round_caps: !self.closed && !self.settings.thick && self.settings.cap == PipeCap::Round,
        })
    }
}

fn interpolate(sections: &[(Real, [Real; 2])], f: Real, blend: SweepBlend) -> [Real; 2] {
    let index = sections.partition_point(|&(t, _)| t < f);
    if index == 0 {
        return sections[0].1;
    }
    if index == sections.len() {
        return sections[index - 1].1;
    }
    let (a, x) = sections[index - 1];
    let (b, y) = sections[index];
    let t = (f - a) / (b - a);
    let t = if blend == SweepBlend::Local {
        t * t * (3. - 2. * t)
    } else {
        t
    };
    std::array::from_fn(|wall| x[wall] * (1. - t) + y[wall] * t)
}
