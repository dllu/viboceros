//! Staged component subdivision: accepted points survive finishing with Escape.
use super::*;

const USAGE: &str = "SplitEdge object-id edge-index parameter [parameter ...]";

#[derive(Clone, Debug)]
struct DistanceConstraint {
    length: Real,
    parameters: Vec<Real>,
}

/// A read-only source snapshot and collected split parameters. No document
/// transaction is held while the user picks points or edits a construction plane.
#[derive(Clone, Debug)]
pub struct SplitEdgeSelection {
    object: ObjectId,
    edge: usize,
    source: Geometry,
    brep: Brep,
    tolerance: Tolerance,
    parameters: Vec<Real>,
    distance: Option<DistanceConstraint>,
}

impl SplitEdgeSelection {
    pub fn prepare(
        document: &Document,
        object: ObjectId,
        edge: usize,
    ) -> Result<Self, CommandError> {
        if !document.is_object_selectable(object) {
            return Err(CommandError::SplitEdgeUnavailable);
        }
        let source = document
            .object(object)
            .ok_or(CommandError::SplitEdgeUnavailable)?
            .geometry()
            .clone();
        let tolerance = document.tolerance();
        let brep = match &source {
            Geometry::Brep(brep) => brep.clone(),
            Geometry::NurbsSurface(surface) => Brep::try_surface_face(surface.clone(), tolerance)?,
            _ => return Err(CommandError::SplitEdgeUnavailable),
        };
        if edge >= brep.edges().len() {
            return Err(CommandError::SplitEdgeUnavailable);
        }
        Ok(Self {
            object,
            edge,
            source,
            brep,
            tolerance,
            parameters: Vec::new(),
            distance: None,
        })
    }

    pub fn object(&self) -> ObjectId {
        self.object
    }
    pub fn edge(&self) -> usize {
        self.edge
    }
    pub fn curve(&self) -> &NurbsCurve {
        self.brep.edges()[self.edge].curve()
    }
    pub fn parameters(&self) -> &[Real] {
        &self.parameters
    }

    /// Cached candidates for the next point. `None` means unconstrained; an
    /// empty slice means the active length cannot be reached in either direction.
    pub fn distance_parameters(&self) -> Option<&[Real]> {
        self.distance.as_ref().map(|d| d.parameters.as_slice())
    }

    pub fn distance(&self) -> Option<Real> {
        self.distance.as_ref().map(|d| d.length)
    }

    /// Zero clears the constraint; signed input uses its magnitude. Integration
    /// happens here and after accepted points, never during viewport hover.
    pub fn set_distance(&mut self, distance: Real) -> Result<(), CommandError> {
        if !distance.is_finite() {
            return Err(CommandError::InvalidNumber(distance.to_string()));
        }
        if distance == 0.0 {
            self.distance = None;
            return Ok(());
        }
        let anchor = *self
            .parameters
            .last()
            .ok_or(CommandError::SplitEdgeDistanceAnchor)?;
        let parameters = distance_parameters(self.curve(), anchor, distance.abs(), self.tolerance)?;
        self.distance = Some(DistanceConstraint {
            length: distance.abs(),
            parameters,
        });
        Ok(())
    }

    pub fn validate_source(&self, document: &Document) -> Result<(), CommandError> {
        if !document.is_object_selectable(self.object)
            || document.tolerance() != self.tolerance
            || document
                .object(self.object)
                .is_none_or(|o| o.geometry() != &self.source)
        {
            return Err(CommandError::SplitEdgeStale);
        }
        Ok(())
    }

    /// Typed model points are constrained to the original edge, not its control
    /// polygon or an arbitrary construction plane. This uses the kernel's bounded
    /// closest-parameter search, not a certified global rational minimum.
    pub fn add_point(&mut self, point: Point3) -> Result<(), CommandError> {
        if let Some(distance) = &self.distance {
            let mut best: Option<(Real, Real)> = None;
            for &parameter in &distance.parameters {
                let score = self.curve().evaluate(parameter)?.distance_to(point)?;
                if best.is_none_or(|(old, _)| score < old) {
                    best = Some((score, parameter));
                }
            }
            // An unreachable constraint leaves both the batch and its anchor intact.
            return best.map_or(Ok(()), |(_, parameter)| self.add_parameter(parameter));
        }
        self.add_parameter(self.curve().closest_parameter(point, self.tolerance)?)
    }

    /// Collect a finite in-domain location without changing the document. The
    /// whole batch is subdivided only on finishing, as in the measured command.
    pub fn add_parameter(&mut self, parameter: Real) -> Result<(), CommandError> {
        if self.parameters.len() >= 100_000 {
            return Err(GeometryError::InvalidBrepTopology {
                context: "too many B-rep edge splits",
            }
            .into());
        }
        let domain = self.curve().domain();
        if !parameter.is_finite() || !domain.contains(&parameter) {
            return Err(GeometryError::InvalidCurveSplitParameter.into());
        }
        let next_distance = if let Some(distance) = &self.distance {
            if !distance.parameters.contains(&parameter) {
                return Err(GeometryError::InvalidCurveSplitParameter.into());
            }
            Some(DistanceConstraint {
                length: distance.length,
                parameters: distance_parameters(
                    self.curve(),
                    parameter,
                    distance.length,
                    self.tolerance,
                )?,
            })
        } else {
            None
        };
        self.parameters.push(parameter);
        self.distance = next_distance;
        Ok(())
    }

    pub fn commit(&self, document: &mut Document) -> Result<String, CommandError> {
        run_command_transaction(document, "SplitEdge", |document| self.apply(document))
    }

    fn apply(&self, document: &mut Document) -> Result<String, CommandError> {
        self.validate_source(document)?;
        if self.parameters.is_empty() {
            return Ok("No edge splits".into());
        }
        let domain = self.curve().domain();
        let parameters: Vec<_> = self
            .parameters
            .iter()
            .copied()
            .filter(|t| *t > *domain.start() && *t < *domain.end())
            .collect();
        let splits = if parameters.is_empty() {
            Vec::new()
        } else {
            vec![(self.edge, parameters)]
        };
        let result = self
            .brep
            .try_split_edges_at_parameters(&splits, self.tolerance)?;
        let count = result.edges().len() - self.brep.edges().len();
        document.clear_selection();
        document.replace_object_geometries_with_history(
            [(self.object, Geometry::Brep(result))],
            viboceros_document::ReplacementHistory::EveryReplacement,
        )?;
        Ok(format!("Split edge at {count} point(s)"))
    }
}

fn distance_parameters(
    curve: &NurbsCurve,
    anchor: Real,
    length: Real,
    tolerance: Tolerance,
) -> Result<Vec<Real>, GeometryError> {
    let mut parameters = Vec::with_capacity(2);
    let closed = curve.is_closed()?;
    let domain = curve.domain();
    for direction in [-1., 1.] {
        let mut candidate =
            curve.parameter_at_arc_length_from(anchor, direction * length, tolerance)?;
        if candidate.is_none() && closed {
            // Cross the seam at most once, then stop at the original anchor.
            // Measure the first leg locally rather than subtracting two prefixes.
            let (start, end) = (*domain.start(), *domain.end());
            let (first, second, next) = if direction > 0. {
                (anchor..=end, start..=anchor, start)
            } else {
                (start..=anchor, anchor..=end, end)
            };
            let consumed = if first.start() == first.end() {
                0.
            } else {
                curve.try_trimmed(first)?.length(tolerance)?
            };
            if second.start() < second.end() && length >= consumed {
                candidate = curve.try_trimmed(second)?.parameter_at_arc_length_from(
                    next,
                    direction * (length - consumed),
                    tolerance,
                )?;
            }
        }
        if let Some(parameter) = candidate
            && !parameters.contains(&parameter)
        {
            parameters.push(parameter);
        }
    }
    Ok(parameters)
}

pub(super) struct SplitEdgeCommand;
impl Command for SplitEdgeCommand {
    fn name(&self) -> &'static str {
        "SplitEdge"
    }
    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        if arguments.len() < 3 {
            return Err(CommandError::Usage(USAGE));
        }
        let object = arguments[0]
            .parse()
            .map_err(|_| CommandError::Usage(USAGE))?;
        let edge = arguments[1]
            .parse()
            .map_err(|_| CommandError::Usage(USAGE))?;
        let mut selection = SplitEdgeSelection::prepare(document, object, edge)?;
        for value in &arguments[2..] {
            selection.add_parameter(parse_finite_real(value)?)?;
        }
        selection.apply(document)
    }
}

#[cfg(test)]
mod tests;
