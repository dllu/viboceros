//! Defining-point extraction stages owned locations while borrowing input geometry.
use super::*;

mod options;
use options::{ExtractPointOptions, ExtractPointOutput, ExtractPointOutputLayer};
#[cfg(test)]
mod tests;

const USAGE: &str = "ExtractPt [OutputLayer=Input|Current] [Output=Points|PointCloud]";
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
        Ok(Some(
            options::parse_extract_point_arguments(arguments)?.prompt(),
        ))
    }

    fn cancel_empty_object_selection(&self) -> bool {
        true
    }

    fn run(&self, document: &mut Document, arguments: &[&str]) -> Result<String, CommandError> {
        let options = options::parse_extract_point_arguments(arguments)?;
        Extraction::prepare(document, options)?.apply(document)
    }
}

struct PointBatch {
    points: Vec<Point3>,
    attributes: ObjectAttributes,
}

enum Output {
    Points(Vec<PointBatch>),
    PointCloud(Vec<Point3>, ObjectAttributes),
}

struct Extraction {
    output: Output,
    selected: usize,
    contributors: usize,
    count: usize,
}

impl Extraction {
    fn prepare(document: &Document, options: ExtractPointOptions) -> Result<Self, CommandError> {
        let current = || ObjectAttributes::on_layer(document.current_layer_id());
        let mut output = match options.output {
            ExtractPointOutput::Points => Output::Points(Vec::new()),
            ExtractPointOutput::PointCloud => Output::PointCloud(Vec::new(), current()),
        };
        let mut selected = 0;
        let mut contributors = 0;
        let mut count = 0usize;
        for object in document.selected_objects() {
            selected += 1;
            let points = object.geometry().extract_point_locations()?;
            if points.is_empty() {
                continue;
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
                    attributes: match options.output_layer {
                        ExtractPointOutputLayer::Input => object.attributes().clone(),
                        ExtractPointOutputLayer::Current => current(),
                    },
                }),
                Output::PointCloud(all, attributes) => {
                    // Only a contributing first source supplies cloud attributes;
                    // otherwise retain the fresh current-layer fallback.
                    if selected == 1 && options.output_layer == ExtractPointOutputLayer::Input {
                        *attributes = object.attributes().clone();
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
        }
        if selected == 0 {
            return Err(CommandError::NoObjectsSelected);
        }
        if count == 0 {
            return Err(CommandError::NoExtractablePoints);
        }
        Ok(Self {
            output,
            selected,
            contributors,
            count,
        })
    }

    fn apply(self, document: &mut Document) -> Result<String, CommandError> {
        let mut ids = Vec::new();
        match self.output {
            Output::Points(batches) => {
                ids.reserve(self.count);
                for batch in batches {
                    for point in batch.points {
                        ids.push(document.add_geometry_with_attributes(
                            Geometry::Point(point),
                            batch.attributes.clone(),
                        )?);
                    }
                }
            }
            Output::PointCloud(points, attributes) => {
                let cloud = PointCloud3::try_new(points)?;
                ids.push(
                    document
                        .add_geometry_with_attributes(Geometry::PointCloud(cloud), attributes)?,
                );
            }
        }
        replace_selection(document, ids)?;
        Ok(format!(
            "Extracted {} point(s) from {} of {} selected object(s)",
            self.count, self.contributors, self.selected
        ))
    }
}
