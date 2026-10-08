//! Surface branch of Rebuild, independent of curve option parsing.
use super::*;
use viboceros_geometry::try_rebuild_nurbs_surface;

const USAGE: &str = "Rebuild UPointCount=2..256 VPointCount=2..256 UDegree=1..11 VDegree=1..11 [DeleteInput=Yes|No] [OutputLayer=Input|Current] [ReTrim=Yes|No]";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Options {
    pub count: [usize; 2],
    pub degree: [usize; 2],
    pub delete: bool,
    pub current: bool,
    pub retrim: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            count: [10; 2],
            degree: [3; 2],
            delete: true,
            current: false,
            retrim: true,
        }
    }
}
impl Options {
    pub fn command_line(self) -> String {
        format!(
            "Rebuild UDegree={} VDegree={} UPointCount={} VPointCount={} DeleteInput={} OutputLayer={} ReTrim={}",
            self.degree[0],
            self.degree[1],
            self.count[0],
            self.count[1],
            if self.delete { "Yes" } else { "No" },
            if self.current { "Current" } else { "Input" },
            if self.retrim { "Yes" } else { "No" }
        )
    }
}

/// Apply scripted options in order. Raising degree raises its count; a count
/// below the current degree minimum is rejected without changing defaults.
pub fn parse(args: &[&str], mut result: Options) -> Result<Options, CommandError> {
    let mut seen = BTreeSet::new();
    let mut index = 0;
    while index < args.len() {
        let (name, value, consumed) = if let Some((name, value)) = args[index].split_once('=') {
            (name, value, 1)
        } else {
            (
                args[index],
                *args.get(index + 1).ok_or(CommandError::Usage(USAGE))?,
                2,
            )
        };
        index += consumed;
        let name = name.trim_start_matches('_').to_ascii_lowercase();
        let value = value.trim_start_matches('_');
        if !seen.insert(name.clone()) {
            return Err(CommandError::Usage(USAGE));
        }
        match name.as_str() {
            "upointcount" | "vpointcount" | "udegree" | "vdegree" => {
                let axis = usize::from(name.starts_with('v'));
                let number = value
                    .parse::<usize>()
                    .map_err(|_| CommandError::InvalidInteger(value.into()))?;
                if name.ends_with("degree") {
                    if !(1..=11).contains(&number) {
                        return Err(CommandError::Usage(USAGE));
                    }
                    result.degree[axis] = number;
                    result.count[axis] = result.count[axis].max(number + 1);
                } else {
                    if number <= result.degree[axis] || number > 256 {
                        return Err(CommandError::Usage(USAGE));
                    }
                    result.count[axis] = number;
                }
            }
            "deleteinput" => {
                result.delete = parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?
            }
            "retrim" => result.retrim = parse_yes_no(value).ok_or(CommandError::Usage(USAGE))?,
            "outputlayer" => {
                result.current = match value.to_ascii_lowercase().as_str() {
                    "current" | "currentlayer" => true,
                    "input" | "inputobject" | "inputobjects" => false,
                    _ => return Err(CommandError::Usage(USAGE)),
                }
            }
            _ => return Err(CommandError::Usage(USAGE)),
        }
    }
    if (0..2).any(|axis| {
        !(1..=11).contains(&result.degree[axis])
            || result.count[axis] <= result.degree[axis]
            || result.count[axis] > 256
    }) {
        return Err(CommandError::Usage(USAGE));
    }
    Ok(result)
}

/// Source admission state retained independently of prepared geometry.
#[derive(Clone, Debug)]
pub struct SourceState {
    objects: Vec<viboceros_document::Object>,
    tolerance: Tolerance,
    current_layer: viboceros_document::LayerId,
}
impl SourceState {
    pub fn capture(doc: &Document) -> Self {
        Self {
            objects: doc.selected_objects().cloned().collect(),
            tolerance: doc.tolerance(),
            current_layer: doc.current_layer_id(),
        }
    }
    pub fn is_current(&self, doc: &Document) -> bool {
        self.tolerance == doc.tolerance()
            && self.current_layer == doc.current_layer_id()
            && doc.selected_objects().count() == self.objects.len()
            && doc.selected_objects().zip(&self.objects).all(|(a, b)| {
                doc.is_object_selectable(a.id())
                    && a.geometry_snapshot()
                        .shares_storage_with(b.geometry_snapshot())
                    && a == b
            })
    }
}

/// Readonly result preparation shared by scripts, preview and acceptance.
#[derive(Clone, Debug)]
pub struct Prepared {
    state: SourceState,
    options: Options,
    output: Vec<(
        ObjectId,
        viboceros_document::GeometrySnapshot,
        ObjectAttributes,
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
        self.output.iter().map(|(_, geometry, _)| geometry)
    }
    /// Reuse geometry when only deletion or layer policy changes.
    pub fn update_output_options(
        &mut self,
        doc: &Document,
        options: Options,
    ) -> Result<(), CommandError> {
        if !self.is_current(doc)
            || options.count != self.options.count
            || options.degree != self.options.degree
            || options.retrim != self.options.retrim
        {
            return Err(CommandError::StaleSurfaceRebuild);
        }
        for (id, _, attrs) in &mut self.output {
            let source = doc.object(*id).ok_or(CommandError::StaleSurfaceRebuild)?;
            *attrs = source.attributes().clone().with_layer(if options.current {
                doc.current_layer_id()
            } else {
                source.attributes().layer_id()
            });
        }
        self.options = options;
        Ok(())
    }
    /// Caller groups all edits in one transaction. Validate the complete input
    /// state before touching any output; geometry is never rebuilt here.
    pub fn apply(&self, doc: &mut Document) -> Result<String, CommandError> {
        if !self.is_current(doc) {
            return Err(CommandError::StaleSurfaceRebuild);
        }
        for (id, geometry, attrs) in &self.output {
            if self.options.delete {
                doc.replace_object_geometries([(*id, (**geometry).clone())])?;
                doc.set_objects_layer([*id], attrs.layer_id())?;
                doc.clear_object_group_memberships([*id])?;
            } else {
                doc.add_geometry_with_attributes((**geometry).clone(), attrs.clone())?;
            }
        }
        doc.clear_selection();
        Ok(format!("Rebuilt {} surface(s)", self.output.len()))
    }
}

pub fn prepare(doc: &Document, options: Options) -> Result<Prepared, CommandError> {
    if (0..2).any(|axis| {
        !(1..=11).contains(&options.degree[axis])
            || options.count[axis] <= options.degree[axis]
            || options.count[axis] > 256
    }) {
        return Err(CommandError::Usage(USAGE));
    }
    let state = SourceState::capture(doc);
    if !state.is_current(doc) {
        return Err(CommandError::StaleSurfaceRebuild);
    }
    if doc
        .selected_objects()
        .any(|object| object.geometry().curve_ref().is_some())
    {
        return Err(CommandError::UnsupportedSurfaceRebuild);
    }
    let mut control_budget = 0usize;
    for object in doc.selected_objects() {
        let source = match object.geometry() {
            Geometry::NurbsSurface(s) => s,
            Geometry::Brep(b) if b.faces().len() == 1 => b.faces()[0].surface(),
            Geometry::Brep(_) => return Err(CommandError::UnsupportedSurfaceRebuild),
            _ => continue,
        };
        let closed = [source.is_closed_u()?, source.is_closed_v()?];
        let count = std::array::from_fn::<_, 2, _>(|axis| {
            options.count[axis]
                + if closed[axis] {
                    options.degree[axis]
                } else {
                    0
                }
        });
        control_budget = control_budget
            .checked_add(count[0] * count[1])
            .ok_or(CommandError::Usage(USAGE))?;
        if control_budget > 1_000_000 {
            return Err(CommandError::Usage(USAGE));
        }
    }
    let mut output = Vec::new();
    for object in doc.selected_objects() {
        let source = match object.geometry() {
            Geometry::NurbsSurface(s) => s,
            Geometry::Brep(b) if b.faces().len() == 1 => b.faces()[0].surface(),
            Geometry::Brep(_) => return Err(CommandError::UnsupportedSurfaceRebuild),
            _ => continue,
        };
        let rebuilt = try_rebuild_nurbs_surface(source, options.count, options.degree)?;
        let brep = if options.retrim {
            match object.geometry() {
                Geometry::Brep(b) => b.try_retrimmed_single_surface(rebuilt, doc.tolerance())?,
                _ => Brep::try_surface_face(source.clone(), doc.tolerance())?
                    .try_retrimmed_single_surface(rebuilt, doc.tolerance())?,
            }
        } else {
            let b = Brep::try_surface_face(rebuilt, doc.tolerance())?;
            let b = if matches!(object.geometry(),Geometry::Brep(original) if original.faces()[0].is_reversed())
            {
                b.reversed()
            } else {
                b
            };
            // Removing trims can close an inward open source into a full solid.
            // Native Rebuild normalizes that output's orientation. The volume
            // fallback applies only to this one connected, single-face shell.
            match b.solid_orientation()? {
                viboceros_geometry::BrepSolidOrientation::Inward => b.reversed(),
                viboceros_geometry::BrepSolidOrientation::Unknown
                    if b.signed_volume(doc.tolerance())? < 0. =>
                {
                    b.reversed()
                }
                _ => b,
            }
        };
        let attrs = object.attributes().clone().with_layer(if options.current {
            doc.current_layer_id()
        } else {
            object.attributes().layer_id()
        });
        output.push((
            object.id(),
            viboceros_document::GeometrySnapshot::from(Geometry::Brep(brep)),
            attrs,
        ));
    }
    if output.is_empty() {
        return Err(CommandError::UnsupportedSurfaceRebuild);
    }
    Ok(Prepared {
        state,
        options,
        output,
    })
}

pub(super) fn run(doc: &mut Document, options: Options) -> Result<String, CommandError> {
    prepare(doc, options)?.apply(doc)
}

impl CommandRegistry {
    pub fn surface_rebuild_defaults(&self) -> Options {
        self.surface_rebuild_preferences.get()
    }
    pub fn remember_surface_rebuild_options(&self, options: Options) {
        self.surface_rebuild_preferences.set(options);
    }
}

#[cfg(test)]
mod retrim_tests;
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn surfaces_rebuild_native_controls_metadata_identity_selection_and_history() {
        let q: serde_json::Value = serde_json::from_str(include_str!(
            "../../../tools/rhino_oracle/observations/surface_rebuild.json"
        ))
        .unwrap();
        for row in q["results"].as_array().unwrap() {
            let v = &row["value"];
            let mut doc = Document::default();
            let registry = CommandRegistry::with_builtins();
            let layers = (0..3)
                .map(|i| {
                    doc.add_layer(format!("Layer {i}"), ColorRgb::BLACK)
                        .unwrap()
                })
                .collect::<Vec<_>>();
            doc.set_current_layer(layers[2]).unwrap();
            let source =
                crate::tween_surfaces::tests::native_surface(&v["before"][0]["definition"]);
            let attrs = ObjectAttributes::on_layer(layers[0])
                .with_name("source-0")
                .with_object_color(ColorRgb::new(20, 40, 60))
                .try_with_user_text("Code", "attribute-0")
                .unwrap();
            let id = doc
                .add_geometry_with_attributes(
                    Geometry::Brep(Brep::try_surface_face(source, doc.tolerance()).unwrap()),
                    attrs,
                )
                .unwrap();
            let groups = (0..2)
                .map(|_| doc.add_group(None, [id]).unwrap())
                .collect::<Vec<_>>();
            doc.clear_history().unwrap();
            let before = doc.objects().cloned().collect::<Vec<_>>();
            doc.select_objects_direct([id], SelectionMode::Replace)
                .unwrap();
            let spec = &v["spec"];
            registry.execute(&mut doc,&format!("Rebuild UPointCount={} VPointCount={} UDegree={} VDegree={} DeleteInput={} OutputLayer={} ReTrim=No",
                spec["count"][0],spec["count"][1],spec["degree"][0],spec["degree"][1],
                if spec["delete"]==true {"Yes"}else{"No"},if spec["current"]==true {"Current"}else{"Input"})).unwrap();
            let native = v["command"]["after_script"].as_array().unwrap();
            assert_eq!(doc.objects().len(), native.len());
            for (object, n) in doc.objects().zip(native) {
                assert_eq!(
                    Some(object.id() == id),
                    n["source"].as_u64().map(|i| i == 0).or(Some(false))
                );
                let a = object.attributes();
                assert_eq!(a.name(), n["name"].as_str());
                assert_eq!(
                    layers.iter().position(|id| *id == a.layer_id()).unwrap(),
                    n["layer"].as_u64().unwrap() as usize
                );
                assert_eq!(a.object_color(), ColorRgb::new(20, 40, 60));
                assert_eq!(a.user_text().get("Code").unwrap(), "attribute-0");
                assert!(object.geometry_user_text().is_empty());
                assert_eq!(
                    doc.is_selected(object.id()),
                    n["selected"].as_bool().unwrap()
                );
                assert_eq!(
                    object
                        .group_ids()
                        .iter()
                        .map(|id| groups.iter().position(|g| g == id).unwrap())
                        .collect::<Vec<_>>(),
                    serde_json::from_value::<Vec<usize>>(n["groups"].clone()).unwrap()
                );
                let Geometry::Brep(b) = object.geometry() else {
                    panic!()
                };
                let s = b.faces()[0].surface();
                let expected = crate::tween_surfaces::tests::native_surface(&n["definition"]);
                assert_eq!(s.knots_u(), expected.knots_u());
                assert_eq!(s.knots_v(), expected.knots_v());
                for (a, b) in s.control_points().iter().zip(expected.control_points()) {
                    assert!(
                        a.point().distance_to(b.point()).unwrap() < 1e-6,
                        "{}",
                        v["case"]
                    );
                }
            }
            let accepted = doc.objects().cloned().collect::<Vec<_>>();
            registry.execute(&mut doc, "Undo").unwrap();
            assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
            registry.execute(&mut doc, "Redo").unwrap();
            assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), accepted);
        }
    }
    #[test]
    fn invalid_and_failed_surface_preparation_preserve_all_sources_and_history() {
        let registry = CommandRegistry::with_builtins();
        let mut doc = Document::default();
        let surface = NurbsSurface::try_bilinear([
            Point3::try_new(0., 0., 0.).unwrap(),
            Point3::try_new(4., 0., 0.).unwrap(),
            Point3::try_new(4., 6., 0.).unwrap(),
            Point3::try_new(0., 6., 0.).unwrap(),
        ])
        .unwrap();
        let id = doc.add_geometry(Geometry::NurbsSurface(surface)).unwrap();
        doc.select_objects_direct([id], SelectionMode::Replace)
            .unwrap();
        doc.clear_history().unwrap();
        let before = doc.objects().cloned().collect::<Vec<_>>();
        for line in [
            "Rebuild UPointCount=257",
            "Rebuild UDegree=12",
            "Rebuild UPointCount=3 UDegree=3",
            "Rebuild ReTrim=Maybe",
            "Rebuild UPointCount=5 UPointCount=6",
            "Rebuild PointCount=5,4",
        ] {
            assert!(registry.execute(&mut doc, line).is_err());
            assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
            assert!(!doc.can_undo());
        }
    }
    #[test]
    fn trimmed_rebuild_retains_holes_and_explicit_untrim_restores_full_surface() {
        let registry = CommandRegistry::with_builtins();
        let mut doc = Document::default();
        let normal = UnitVector3::try_new(0., 0., 1., doc.tolerance()).unwrap();
        let center = Point3::try_new(0., 0., 0.).unwrap();
        let outer = Circle3::try_new(center, 5., normal, doc.tolerance())
            .unwrap()
            .to_nurbs()
            .unwrap();
        let inner = Circle3::try_new(center, 2., normal, doc.tolerance())
            .unwrap()
            .to_nurbs()
            .unwrap();
        let brep = Brep::try_planar_face_with_holes(&outer, &[inner], doc.tolerance()).unwrap();
        let id = doc.add_geometry(Geometry::Brep(brep)).unwrap();
        let another = doc
            .add_geometry(Geometry::NurbsSurface(
                NurbsSurface::try_bilinear([
                    Point3::try_new(10., 0., 0.).unwrap(),
                    Point3::try_new(14., 0., 0.).unwrap(),
                    Point3::try_new(14., 6., 0.).unwrap(),
                    Point3::try_new(10., 6., 0.).unwrap(),
                ])
                .unwrap(),
            ))
            .unwrap();
        doc.select_objects_direct([another, id], SelectionMode::Replace)
            .unwrap();
        doc.clear_history().unwrap();
        let before = doc.objects().cloned().collect::<Vec<_>>();
        registry.execute(&mut doc, "Rebuild").unwrap();
        let Geometry::Brep(rebuilt) = doc.object(id).unwrap().geometry() else {
            panic!()
        };
        assert_eq!(rebuilt.faces()[0].loops().len(), 2);
        assert!((rebuilt.area(doc.tolerance()).unwrap() - 21. * std::f64::consts::PI).abs() < 1e-7);
        registry.execute(&mut doc, "Undo").unwrap();
        assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
        registry
            .execute(&mut doc, "Rebuild ReTrim=No UPointCount=4 VPointCount=4")
            .unwrap();
        assert_eq!(doc.objects().len(), 2);
        let Geometry::Brep(b) = doc.object(id).unwrap().geometry() else {
            panic!()
        };
        assert!(b.faces()[0].is_untrimmed(doc.tolerance()).unwrap());
        registry.execute(&mut doc, "Undo").unwrap();
        assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
    }
    #[test]
    fn surface_preferences_follow_ordered_degree_growth_and_survive_history() {
        let registry = CommandRegistry::with_builtins();
        assert_eq!(registry.surface_rebuild_defaults(), Options::default());
        let options = parse(
            &["UDegree=1", "UPointCount=2", "VDegree=1", "VPointCount=2"],
            Options::default(),
        )
        .unwrap();
        assert_eq!(options.count, [2, 2]);
        assert_eq!(options.degree, [1, 1]);
        let raised = parse(&["UPointCount=3", "UDegree=4"], options).unwrap();
        assert_eq!(raised.count, [5, 2]);
        assert_eq!(raised.degree, [4, 1]);
        assert!(parse(&["UDegree=4", "UPointCount=3"], options).is_err());
        let mut doc = Document::default();
        let s = NurbsSurface::try_bilinear([
            Point3::try_new(0., 0., 0.).unwrap(),
            Point3::try_new(4., 0., 0.).unwrap(),
            Point3::try_new(4., 6., 0.).unwrap(),
            Point3::try_new(0., 6., 0.).unwrap(),
        ])
        .unwrap();
        let id = doc.add_geometry(Geometry::NurbsSurface(s)).unwrap();
        doc.select_objects_direct([id], SelectionMode::Replace)
            .unwrap();
        registry
            .execute(
                &mut doc,
                "Rebuild UDegree=1 VDegree=1 UPointCount=2 VPointCount=2",
            )
            .unwrap();
        assert_eq!(registry.surface_rebuild_defaults(), options);
        registry.execute(&mut doc, "Undo").unwrap();
        registry.execute(&mut doc, "Redo").unwrap();
        assert_eq!(registry.surface_rebuild_defaults(), options);
        assert_eq!(
            CommandRegistry::with_builtins().surface_rebuild_defaults(),
            Options::default()
        );
        registry
            .execute(&mut doc, "RebuildCrv PointCount=6 Degree=2")
            .unwrap_err();
        assert_eq!(registry.surface_rebuild_defaults(), options);
    }
    #[test]
    fn prepared_rebuild_is_readonly_reuses_output_geometry_and_rejects_stale_sources() {
        let registry = CommandRegistry::with_builtins();
        let mut doc = Document::default();
        registry
            .execute(&mut doc, "SrfPt 0,0,0 4,0,0 4,6,2 0,6,0")
            .unwrap();
        let id = doc.objects().next().unwrap().id();
        doc.select_objects_direct([id], SelectionMode::Replace)
            .unwrap();
        doc.clear_history().unwrap();
        doc.add_geometry(Geometry::Point(Point3::try_new(9., 8., 7.).unwrap()))
            .unwrap();
        doc.undo().unwrap();
        assert!(doc.can_redo());
        let before = doc.objects().cloned().collect::<Vec<_>>();
        let mut prepared = prepare(
            &doc,
            Options {
                count: [5, 4],
                degree: [3, 2],
                retrim: false,
                ..Options::default()
            },
        )
        .unwrap();
        assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
        assert!(!doc.can_undo());
        assert!(doc.can_redo());
        let geometry = prepared.outputs().next().unwrap().clone();
        prepared
            .update_output_options(
                &doc,
                Options {
                    delete: false,
                    current: true,
                    ..prepared.options()
                },
            )
            .unwrap();
        assert!(geometry.shares_storage_with(prepared.outputs().next().unwrap()));
        assert!(doc.can_redo());
        for change in 0..5 {
            let mut changed = doc.clone();
            match change {
                0 => {
                    changed
                        .set_object_names([(id, Some("Changed".into()))])
                        .unwrap();
                }
                1 => {
                    changed.add_group(None, [id]).unwrap();
                }
                2 => {
                    changed.set_tolerance(Tolerance::try_new(1e-5, 1e-12, 1e-10).unwrap());
                }
                3 => {
                    changed.clear_selection();
                }
                _ => {
                    changed
                        .replace_object_geometries([(
                            id,
                            Geometry::Point(Point3::try_new(0., 0., 0.).unwrap()),
                        )])
                        .unwrap();
                }
            }
            let before = changed.objects().cloned().collect::<Vec<_>>();
            assert!(matches!(
                prepared.apply(&mut changed),
                Err(CommandError::StaleSurfaceRebuild)
            ));
            assert_eq!(changed.objects().cloned().collect::<Vec<_>>(), before);
        }
        assert!(
            prepare(
                &doc,
                Options {
                    count: [0, 4],
                    ..Options::default()
                }
            )
            .is_err()
        );
    }
    #[test]
    fn periodic_control_repetitions_count_toward_the_preparation_budget() {
        let mut doc = Document::default();
        let tolerance = doc.tolerance();
        let frame = Frame3::try_from_normal(
            Point3::try_new(0., 0., 0.).unwrap(),
            Vector3::try_new(0., 0., 1.).unwrap(),
            tolerance,
        )
        .unwrap();
        let torus = NurbsSurface::try_torus(frame, 4., 1.).unwrap();
        let ids = (0..15)
            .map(|_| {
                doc.add_geometry(Geometry::NurbsSurface(torus.clone()))
                    .unwrap()
            })
            .collect::<Vec<_>>();
        doc.select_objects_direct(ids, SelectionMode::Replace)
            .unwrap();
        doc.clear_history().unwrap();
        let before = doc.objects().cloned().collect::<Vec<_>>();
        // 15*256^2 fits, but the actual 15*259^2 periodic nets exceed one million.
        assert!(
            prepare(
                &doc,
                Options {
                    count: [256, 256],
                    ..Options::default()
                }
            )
            .is_err()
        );
        assert_eq!(doc.objects().cloned().collect::<Vec<_>>(), before);
        assert!(!doc.can_undo());
    }
}
