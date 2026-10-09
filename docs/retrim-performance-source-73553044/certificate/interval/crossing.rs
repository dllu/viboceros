//! Enclosures of the exact restricted patch boxes at tensor-knot crossings.
use super::*;

pub(in crate::surface_pullback::certificate) struct Boxes {
    image: Option<[[I; 2]; 3]>,
    work: Work,
}
impl Boxes {
    pub fn new() -> Self {
        Self {
            image: None,
            work: Work {
                remaining: FLOAT_WORK,
                coefficients: BTreeMap::new(),
            },
        }
    }
    pub fn add(
        &mut self,
        net: &[H],
        degree: [usize; 2],
        intervals: &[[Rational; 2]],
    ) -> Option<()> {
        let convert = |p: &H| {
            Some([
                I::exact(&p[0])?,
                I::exact(&p[1])?,
                I::exact(&p[2])?,
                I::exact(&p[3])?,
            ])
        };
        let mut result = net.iter().map(convert).collect::<Option<Vec<_>>>()?;
        let u = [I::exact(&intervals[0][0])?, I::exact(&intervals[0][1])?];
        let v = [I::exact(&intervals[1][0])?, I::exact(&intervals[1][1])?];
        for row in result.chunks_mut(degree[0] + 1) {
            row.copy_from_slice(&restrict(row, u, &mut self.work)?);
        }
        for i in 0..=degree[0] {
            let column = (0..=degree[1])
                .map(|j| result[j * (degree[0] + 1) + i])
                .collect::<Vec<_>>();
            let column = restrict(&column, v, &mut self.work)?;
            for (j, point) in column.into_iter().enumerate() {
                result[j * (degree[0] + 1) + i] = point;
            }
        }
        let bounds = boxes(&result)?;
        if let Some(image) = &mut self.image {
            for (old, new) in image.iter_mut().zip(bounds) {
                old[0] = minimum(old[0], new[0]);
                old[1] = maximum(old[1], new[1]);
            }
        } else {
            self.image = Some(bounds);
        }
        Some(())
    }
    pub fn finish(&self, spatial: &[H], limit: Real) -> Crossing {
        self.finish_checked(spatial, limit)
            .unwrap_or(Crossing::Inconclusive)
    }
    fn finish_checked(&self, spatial: &[H], limit: Real) -> Option<Crossing> {
        if !limit.is_finite() || limit < Real::MIN_POSITIVE {
            return None;
        }
        let image = self.image?;
        let spatial = spatial
            .iter()
            .map(|p| {
                Some([
                    I::exact(&p[0])?,
                    I::exact(&p[1])?,
                    I::exact(&p[2])?,
                    I::exact(&p[3])?,
                ])
            })
            .collect::<Option<Vec<_>>>()?;
        let spatial = boxes(&spatial)?;
        let mut difference = [zero(); 4];
        difference[3] = one();
        for axis in 0..3 {
            let a = image[axis][1].sub(spatial[axis][0])?;
            let b = spatial[axis][1].sub(image[axis][0])?;
            let bound = maximum(a, b);
            difference[axis] = I {
                lo: bound.lo.max(0.),
                hi: bound.hi,
            };
        }
        let square = norm_squared(&difference, limit)?;
        if square.hi <= 1. {
            Some(Crossing::Within(upper(square.hi, limit)))
        } else if square.lo > 1. {
            Some(Crossing::Refine)
        } else {
            None
        }
    }
}
fn minimum(a: I, b: I) -> I {
    I {
        lo: a.lo.min(b.lo),
        hi: a.hi.min(b.hi),
    }
}
fn maximum(a: I, b: I) -> I {
    I {
        lo: a.lo.max(b.lo),
        hi: a.hi.max(b.hi),
    }
}
fn boxes(net: &[F]) -> Option<[[I; 2]; 3]> {
    let mut result = None;
    for p in net {
        if p[3].lo <= 0. {
            return None;
        }
        let xyz = [p[0].div(p[3])?, p[1].div(p[3])?, p[2].div(p[3])?];
        if let Some(current) = &mut result {
            let current: &mut [[I; 2]; 3] = current;
            for (axis, value) in current.iter_mut().zip(xyz) {
                axis[0] = minimum(axis[0], value);
                axis[1] = maximum(axis[1], value);
            }
        } else {
            result = Some(xyz.map(|v| [v, v]));
        }
    }
    result
}
fn restrict(net: &[F], interval: [I; 2], work: &mut Work) -> Option<Vec<F>> {
    if interval[0].lo == 0. && interval[0].hi == 0. && interval[1].lo == 1. && interval[1].hi == 1.
    {
        return Some(net.to_vec());
    }
    let degree = net.len() - 1;
    work.charge(4 * (degree + 1).pow(3))?;
    let mut output = Vec::new();
    for right in 0..=degree {
        let mut values = net.to_vec();
        for level in 1..=degree {
            let t = if level <= degree - right {
                interval[0]
            } else {
                interval[1]
            };
            let a = one().sub(t)?;
            for i in 0..=degree - level {
                let next = values[i + 1];
                for (value, next) in values[i].iter_mut().zip(next) {
                    *value = value.mul(a)?.add(next.mul(t)?)?;
                }
            }
        }
        output.push(values[0]);
    }
    Some(output)
}
