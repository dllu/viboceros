//! Size samples on the original parameter domain of a B-rep edge.

#[derive(Clone, Debug, PartialEq)]
pub struct EdgeBlendSizes {
    pub ends: [f64; 2],
    /// Additional sizes in increasing normalized curve-domain order.
    pub interior: Vec<(f64, f64)>,
}

impl From<[f64; 2]> for EdgeBlendSizes {
    fn from(ends: [f64; 2]) -> Self {
        Self {
            ends,
            interior: Vec::new(),
        }
    }
}

impl EdgeBlendSizes {
    pub fn stations(&self) -> impl Iterator<Item = (f64, f64)> + '_ {
        std::iter::once((0., self.ends[0]))
            .chain(self.interior.iter().copied())
            .chain(std::iter::once((1., self.ends[1])))
    }

    pub fn is_valid(&self) -> bool {
        let mut prior = f64::NEG_INFINITY;
        self.stations().all(|(parameter, size)| {
            let ordered = parameter > prior;
            prior = parameter;
            ordered && size.is_finite() && size > 0.
        })
    }

    pub fn all_equal_to(&self, size: f64) -> bool {
        self.stations().all(|(_, value)| value == size)
    }
}
