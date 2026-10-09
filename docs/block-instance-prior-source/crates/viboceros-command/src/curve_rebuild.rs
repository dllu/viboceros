//! Readonly curve Rebuild preparation shared by command execution and previews.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutputLayer {
    Input,
    Current,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Options {
    pub point_count: usize,
    pub degree: usize,
    pub preserve_end_tangents: bool,
    pub delete_input: bool,
    pub output_layer: OutputLayer,
}
use super::*;
use surface_rebuild::SourceState;

impl Default for Options {
    fn default() -> Self {
        Self {
            point_count: 10,
            degree: 3,
            preserve_end_tangents: false,
            delete_input: true,
            output_layer: OutputLayer::Input,
        }
    }
}
impl Options {
    pub fn command_line(self) -> String {
        format!(
            "Rebuild PointCount={} Degree={} PreserveTangents={} DeleteInput={} OutputLayer={}",
            self.point_count,
            self.degree,
            if self.preserve_end_tangents {
                "Yes"
            } else {
                "No"
            },
            if self.delete_input { "Yes" } else { "No" },
            if self.output_layer == OutputLayer::Input {
                "Input"
            } else {
                "Current"
            }
        )
    }
}

/// Apply a partial typed edit without changing any document or registry state.
pub fn parse(args: &[&str], mut options: Options) -> Result<Options, CommandError> {
    let mut seen = BTreeSet::new();
    let mut index = 0;
    while index < args.len() {
        let (name, value, consumed) = if let Some((n, v)) = args[index].split_once('=') {
            (n, v, 1)
        } else {
            (
                args[index],
                *args
                    .get(index + 1)
                    .ok_or(CommandError::Usage(REBUILD_CURVE_USAGE))?,
                2,
            )
        };
        index += consumed;
        let key = match name.trim_start_matches('_').to_ascii_lowercase().as_str() {
            "points" | "pointcount" => "pointcount",
            "degree" => "degree",
            "preservetangents" | "preserveendtangents" => "tangents",
            "deleteinput" => "delete",
            "outputlayer" => "layer",
            _ => return Err(CommandError::Usage(REBUILD_CURVE_USAGE)),
        };
        if !seen.insert(key) {
            return Err(CommandError::Usage(REBUILD_CURVE_USAGE));
        }
        let value = value.trim_start_matches('_');
        match key {
            "pointcount" => {
                options.point_count = value
                    .parse()
                    .map_err(|_| CommandError::InvalidInteger(value.into()))?
            }
            "degree" => {
                options.degree = value
                    .parse()
                    .map_err(|_| CommandError::InvalidInteger(value.into()))?
            }
            "tangents" => {
                options.preserve_end_tangents =
                    parse_yes_no(value).ok_or(CommandError::Usage(REBUILD_CURVE_USAGE))?
            }
            "delete" => {
                options.delete_input =
                    parse_yes_no(value).ok_or(CommandError::Usage(REBUILD_CURVE_USAGE))?
            }
            _ => {
                options.output_layer = match value.to_ascii_lowercase().as_str() {
                    "current" | "currentlayer" => OutputLayer::Current,
                    "input" | "inputobject" | "inputobjects" => OutputLayer::Input,
                    _ => return Err(CommandError::Usage(REBUILD_CURVE_USAGE)),
                }
            }
        }
    }
    if options.degree == 0 || options.degree > MAX_CURVE_REBUILD_DEGREE {
        return Err(GeometryError::InvalidCurveRebuildDegree {
            actual: options.degree,
            maximum: MAX_CURVE_REBUILD_DEGREE,
        }
        .into());
    }
    Ok(options)
}

#[derive(Clone, Debug)]
pub struct Prepared {
    state: SourceState,
    options: Options,
    output: Vec<(
        ObjectId,
        viboceros_document::LayerId,
        viboceros_document::GeometrySnapshot,
    )>,
}
impl Prepared {
    pub fn is_current(&self, doc: &Document) -> bool {
        self.state.is_current(doc)
    }
    pub fn options(&self) -> Options {
        self.options
    }
    pub fn outputs(&self) -> impl ExactSizeIterator<Item = &viboceros_document::GeometrySnapshot> {
        self.output.iter().map(|(_, _, g)| g)
    }
    pub fn update_output_options(
        &mut self,
        doc: &Document,
        options: Options,
    ) -> Result<(), CommandError> {
        if !self.is_current(doc)
            || options.point_count != self.options.point_count
            || options.degree != self.options.degree
            || options.preserve_end_tangents != self.options.preserve_end_tangents
        {
            return Err(CommandError::StaleCurveRebuild);
        }
        self.options = options;
        Ok(())
    }
    /// Caller owns the surrounding transaction; validate all sources first.
    pub fn apply(&self, doc: &mut Document) -> Result<String, CommandError> {
        if !self.is_current(doc) {
            return Err(CommandError::StaleCurveRebuild);
        }
        let o = self.options;
        if o.delete_input && o.output_layer == OutputLayer::Input {
            doc.replace_object_geometries(
                self.output.iter().map(|(id, _, g)| (*id, (**g).clone())),
            )?;
        } else {
            if o.delete_input {
                for (id, _, _) in &self.output {
                    doc.delete_object(*id)?;
                }
            }
            for (_, input, g) in &self.output {
                let layer = if o.output_layer == OutputLayer::Current {
                    doc.current_layer_id()
                } else {
                    *input
                };
                doc.add_geometry_with_attributes((**g).clone(), ObjectAttributes::on_layer(layer))?;
            }
        }
        Ok(format!(
            "Rebuilt {} curve(s) as non-rational degree-{} NURBS with {} requested point(s), {} the input(s) on the {} layer{}",
            self.output.len(),
            o.degree,
            o.point_count,
            if o.delete_input {
                "replacing"
            } else {
                "retaining"
            },
            if o.output_layer == OutputLayer::Current {
                "current"
            } else {
                "input-object"
            },
            if o.preserve_end_tangents {
                " and preserving eligible open-curve end tangents"
            } else {
                ""
            }
        ))
    }
}
pub fn prepare(doc: &Document, options: Options) -> Result<Prepared, CommandError> {
    // Public typed options receive the same checks as scripted options.
    let options = parse(&[], options)?;
    let output = doc
        .selected_objects()
        .filter_map(|o| geometry_curve_ref(o.geometry()).map(|c| (o, c)))
        .map(|(o, c)| {
            Ok((
                o.id(),
                o.attributes().layer_id(),
                viboceros_document::GeometrySnapshot::from(Geometry::NurbsCurve(
                    try_rebuild_curve(
                        c,
                        options.point_count,
                        options.degree,
                        options.preserve_end_tangents,
                        doc.tolerance(),
                    )?,
                )),
            ))
        })
        .collect::<Result<Vec<_>, CommandError>>()?;
    if output.is_empty() {
        return Err(CommandError::NoRebuildCurves);
    }
    Ok(Prepared {
        state: SourceState::capture(doc),
        options,
        output,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use viboceros_document::SelectionMode;
    fn fixture() -> (Document, ObjectId) {
        let mut doc = Document::default();
        let curve = NurbsCurve::try_new(
            3,
            vec![
                Point3::try_new(0., 0., 0.).unwrap(),
                Point3::try_new(1., 2., 0.).unwrap(),
                Point3::try_new(3., -1., 0.).unwrap(),
                Point3::try_new(4., 0., 0.).unwrap(),
            ],
            vec![0., 0., 0., 0., 1., 1., 1., 1.],
        )
        .unwrap();
        let id = doc.add_geometry(Geometry::NurbsCurve(curve)).unwrap();
        doc.select_objects_direct([id], SelectionMode::Replace)
            .unwrap();
        doc.clear_history().unwrap();
        (doc, id)
    }
    #[test]
    fn preparation_is_readonly_reuses_output_policies_and_applies_exact_results() {
        let (mut doc, id) = fixture();
        let before = doc.objects().cloned().collect::<Vec<_>>();
        let mut prepared = prepare(&doc, Options::default()).unwrap();
        let ready = prepared.outputs().next().unwrap().clone();
        assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
        assert!(doc.undo_label().is_none());
        let options = Options {
            delete_input: false,
            ..Options::default()
        };
        prepared.update_output_options(&doc, options).unwrap();
        assert!(ready.shares_storage_with(prepared.outputs().next().unwrap()));
        doc.begin_transaction("Rebuild").unwrap();
        prepared.apply(&mut doc).unwrap();
        doc.commit_transaction().unwrap();
        assert_eq!(doc.object(id).unwrap(), &before[0]);
        assert_eq!(doc.objects().last().unwrap().geometry(), &*ready);
        doc.undo().unwrap();
        assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
        doc.redo().unwrap();
        assert_eq!(doc.objects().last().unwrap().geometry(), &*ready);
    }
    #[test]
    fn source_change_rejects_prepared_acceptance_before_document_edits() {
        let (mut doc, id) = fixture();
        let prepared = prepare(&doc, Options::default()).unwrap();
        doc.set_object_names([(id, Some("changed".into()))])
            .unwrap();
        let before = doc.objects().cloned().collect::<Vec<_>>();
        assert!(matches!(
            prepared.apply(&mut doc),
            Err(CommandError::StaleCurveRebuild)
        ));
        assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
    }
    #[test]
    fn typed_partial_options_retain_unedited_values_and_reject_alias_duplicates() {
        let defaults = Options {
            point_count: 7,
            degree: 2,
            ..Options::default()
        };
        let next = parse(&["Degree", "_3", "PreserveEndTangents=Yes"], defaults).unwrap();
        assert_eq!(next.point_count, 7);
        assert_eq!(next.degree, 3);
        assert!(next.preserve_end_tangents);
        for args in [
            vec!["Points=5", "PointCount=6"],
            vec!["Degree=0"],
            vec!["DeleteInput=Maybe"],
        ] {
            assert!(parse(&args, defaults).is_err());
        }
    }

    #[test]
    fn mixed_sources_prepare_atomically_and_current_layer_outputs_keep_existing_policy() {
        let (mut doc, id) = fixture();
        let input = doc
            .add_layer(
                "Curve source",
                viboceros_document::ColorRgb::new(10, 20, 30),
            )
            .unwrap();
        doc.set_objects_layer([id], input).unwrap();
        doc.set_object_names([(id, Some("named source".into()))])
            .unwrap();
        let second = doc
            .add_geometry(Geometry::Point(Point3::try_new(1., 2., 3.).unwrap()))
            .unwrap();
        doc.select_objects_direct([second], SelectionMode::Add)
            .unwrap();
        doc.clear_history().unwrap();
        let before = doc.objects().cloned().collect::<Vec<_>>();
        let bad = Options {
            point_count: 0,
            ..Options::default()
        };
        assert!(prepare(&doc, bad).is_err());
        assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
        assert!(!doc.can_undo());
        let mut prepared = prepare(&doc, Options::default()).unwrap();
        assert_eq!(prepared.outputs().len(), 1);
        let output = prepared.outputs().next().unwrap().clone();
        prepared
            .update_output_options(
                &doc,
                Options {
                    output_layer: OutputLayer::Current,
                    ..Options::default()
                },
            )
            .unwrap();
        assert!(output.shares_storage_with(prepared.outputs().next().unwrap()));
        doc.begin_transaction("Rebuild").unwrap();
        prepared.apply(&mut doc).unwrap();
        doc.commit_transaction().unwrap();
        assert!(doc.object(id).is_none());
        assert_eq!(doc.object(second).unwrap(), &before[1]);
        let rebuilt = doc.objects().last().unwrap();
        assert_eq!(rebuilt.geometry(), &*output);
        assert_eq!(rebuilt.attributes().layer_id(), doc.current_layer_id());
        assert!(rebuilt.attributes().name().is_none());
        doc.undo().unwrap();
        assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
        doc.redo().unwrap();
        assert_eq!(doc.objects().last().unwrap().geometry(), &*output);
    }
}
