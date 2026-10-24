//! Defining-point extraction stages owned locations while borrowing input geometry.
use super::*;

mod components;
mod inputs;
mod options;
use options::{ExtractPointOptions, ExtractPointOutput, ExtractPointOutputLayer};
#[cfg(test)]
mod tests;

const USAGE: &str = "ExtractPt [OutputLayer=Input|Current] [Output=Points|PointCloud] [Component=uuid,Vertex|Edge|Face,index] [SubDLocation=Surface|ControlNet]";
const MAX_EXTRACTED_POINTS: usize = 1_000_000;

pub(super) struct ExtractPtCommand;

pub(crate) fn accepts(geometry: &Geometry) -> bool {
    !matches!(
        geometry,
        Geometry::Point(_) | Geometry::EmptyMesh | Geometry::BlockInstance(_)
    )
}

impl Command for ExtractPtCommand {
    fn name(&self) -> &'static str {
        "ExtractPt"
    }

    fn object_selection_prompt(
        &self,
        arguments: &[&str],
    ) -> Result<Option<ObjectSelectionPrompt>, CommandError> {
        let options = options::parse_extract_point_arguments(arguments)?;
        Ok(options.components.is_empty().then(|| options.prompt()))
    }

    fn cancel_empty_object_selection(&self) -> bool {
        true
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let options = options::parse_extract_point_arguments(arguments)?;
        Extraction::prepare(document, options, false)?.apply(document)
    }

    fn run_postselected(
        &self,
        document: &mut Document,
        arguments: &[&str],
        _context: CommandContext,
    ) -> Result<String, CommandError> {
        let options = options::parse_extract_point_arguments(arguments)?;
        Extraction::prepare(document, options, true)?.apply(document)
    }
}

struct PointBatch {
    points: Vec<Point3>,
    source: Option<ObjectId>,
}

enum Output {
    Points(Vec<PointBatch>),
    PointCloud(Vec<Point3>, Option<ObjectId>),
}

struct Extraction {
    output: Output,
    selected: usize,
    contributors: usize,
    count: usize,
    retained: Vec<ObjectId>,
    released: Vec<ObjectId>,
    release_grips: bool,
    clear_component_grips: bool,
}

impl Extraction {
    fn prepare(
        document: &Document,
        options: ExtractPointOptions,
        postselected: bool,
    ) -> Result<Self, CommandError> {
        let mut output = match options.output {
            ExtractPointOutput::Points => Output::Points(Vec::new()),
            ExtractPointOutput::PointCloud => Output::PointCloud(Vec::new(), None),
        };
        let mut selected = 0;
        let mut contributors = 0;
        let mut count = 0usize;
        let owners = inputs::visit(document, &options, postselected, |source, points| {
            selected += 1;
            if points.is_empty() {
                return Ok(());
            }
            count = count
                .checked_add(points.len())
                .filter(|&n| n <= MAX_EXTRACTED_POINTS)
                .ok_or(CommandError::TooManyExtractedPoints {
                    maximum: MAX_EXTRACTED_POINTS,
                })?;
            contributors += 1;
            match &mut output {
                Output::Points(batches) => batches.push(PointBatch {
                    points,
                    source: (options.output_layer == ExtractPointOutputLayer::Input)
                        .then_some(source),
                }),
                Output::PointCloud(all, origin) => {
                    // Only a contributing first source supplies cloud attributes;
                    // otherwise retain the fresh current-layer fallback.
                    if selected == 1 && options.output_layer == ExtractPointOutputLayer::Input {
                        *origin = Some(source);
                    }
                    if all.is_empty() {
                        *all = points;
                    } else {
                        all.try_reserve(points.len()).map_err(|_| {
                            CommandError::TooManyExtractedPoints {
                                maximum: MAX_EXTRACTED_POINTS,
                            }
                        })?;
                        all.extend(points);
                    }
                }
            }
            Ok(())
        })?;
        if let Output::PointCloud(_, source) = &mut output
            && options.output_layer == ExtractPointOutputLayer::Input
            && let Some((grip, _)) = document.selected_control_points().next()
        {
            *source = Some(grip.object);
        }
        if selected == 0 {
            return Err(CommandError::NoObjectsSelected);
        }
        if count == 0 {
            return Err(CommandError::NoExtractablePoints);
        }
        Ok(Self {
            clear_component_grips: postselected && !options.components.is_empty(),
            release_grips: postselected,
            output,
            selected,
            contributors,
            count,
            retained: document
                .selected_object_ids()
                .filter(|id| owners.contains(id))
                .collect(),
            released: document
                .selected_object_ids()
                .filter(|id| postselected || !owners.contains(id))
                .collect(),
        })
    }

    fn apply(self, document: &mut Document) -> Result<String, CommandError> {
        let cloud = matches!(&self.output, Output::PointCloud(..));
        document.release_command_selection_on_history_replay(
            self.released
                .into_iter()
                .chain(self.retained.iter().copied().filter(|_| cloud)),
        )?;
        if cloud && !self.clear_component_grips {
            let grips = document
                .selected_control_points()
                .map(|(g, _)| g)
                .collect::<Vec<_>>();
            if self.release_grips {
                document.release_control_point_selection_on_history_replay(grips)?;
            }
        } else {
            document.clear_control_point_selection();
        }
        let mut ids = Vec::new();
        match self.output {
            Output::Points(batches) => {
                ids.reserve(self.count);
                for batch in batches {
                    if let Some(source) = batch.source {
                        ids.extend(
                            document.copy_object_pieces_into_source_groups(
                                batch
                                    .points
                                    .into_iter()
                                    .map(|p| (source, Geometry::Point(p))),
                            )?,
                        );
                    } else {
                        for point in batch.points {
                            ids.push(document.add_geometry(Geometry::Point(point))?);
                        }
                    }
                }
            }
            Output::PointCloud(points, source) => {
                let cloud = PointCloud3::try_new(points)?;
                if let Some(source) = source {
                    ids.extend(document.copy_object_pieces_into_source_groups([(
                        source,
                        Geometry::PointCloud(cloud),
                    )])?);
                } else {
                    ids.push(document.add_geometry(Geometry::PointCloud(cloud))?);
                }
            }
        }
        document.select_command_results(self.retained.into_iter().chain(ids))?;
        Ok(format!(
            "Extracted {} point(s) from {} of {} selected object(s)",
            self.count, self.contributors, self.selected
        ))
    }
}
