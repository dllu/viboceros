//! Staged component subdivision: accepted points survive finishing with Escape.
use super::*;

const USAGE: &str = "SplitEdge object-id edge-index parameter [parameter ...]";

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
        self.parameters.push(parameter);
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
