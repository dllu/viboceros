//! Parameter conditioning for exact piecewise-linear distance sampling.

use super::Polyline3;
use crate::{GeometryError, parameter::map_parameter};
use std::borrow::Cow;

impl Polyline3 {
    pub(crate) fn for_integration(&self) -> Result<Cow<'_, Self>, GeometryError> {
        if self.domain() == (0.0..=1.0) {
            return Ok(Cow::Borrowed(self));
        }
        let parameters = self
            .parameters
            .iter()
            .map(|&parameter| map_parameter(parameter, self.domain(), 0.0..=1.0))
            .collect::<Result<Vec<_>, _>>()?;
        if parameters.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err(GeometryError::NumericalIntegrationDidNotConverge);
        }
        // The source already validated its segments. Copy vertices unchanged,
        // without rechecking them against a potentially different tolerance.
        Ok(Cow::Owned(Self {
            vertices: self.vertices.clone(),
            parameters,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Point3, Tolerance};

    #[test]
    fn conditioning_preserves_validated_vertices_and_rejects_lost_breaks() {
        let vertices = [[0., 0., 0.], [1e-12, 0., 0.], [1e-12, 1e-12, 0.]]
            .map(|p| Point3::try_from(p).unwrap())
            .to_vec();
        let tolerance = Tolerance::try_new(1e-15, 1e-12, 1e-10).unwrap();
        let source =
            Polyline3::try_with_parameters(vertices.clone(), vec![10., 20., 30.], tolerance)
                .unwrap();
        let original = source.clone();
        let normalized = source.for_integration().unwrap();
        assert_eq!(normalized.vertices, vertices);
        assert_eq!(normalized.parameters, vec![0., 0.5, 1.]);
        assert!(matches!(
            normalized.for_integration().unwrap(),
            Cow::Borrowed(_)
        ));
        assert_eq!(source, original);
        let unresolved = Polyline3::try_with_parameters(
            vertices,
            vec![-f64::from_bits(1), 0., f64::MAX],
            tolerance,
        )
        .unwrap();
        assert!(matches!(
            unresolved.for_integration(),
            Err(GeometryError::NumericalIntegrationDidNotConverge)
        ));
    }
}
