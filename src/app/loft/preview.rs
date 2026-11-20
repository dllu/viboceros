//! A completed draft is displayed and then moved into the document on acceptance.
use super::*;
use crate::viewport::curve_seam::Overlay;
use std::sync::Arc;
use viboceros_command::loft;
use viboceros_geometry::{Brep, CurveRef, LoftProfiles, NurbsCurve};

#[derive(Debug)]
pub(super) struct Profiles {
    sources: Vec<CapturedCurve>,
    curves: Vec<NurbsCurve>,
    oriented: bool,
}

pub(super) enum Sections {
    Closed(Seams),
    Open(Profiles),
}

#[derive(Debug)]
pub(super) struct Draft {
    brep: Brep,
    pub(super) wires: Vec<NurbsCurve>,
}

#[derive(Debug)]
pub(super) struct Review {
    profiles: Profiles,
    supports: Arc<[Option<loft::TangentSupport>; 2]>,
    pub(super) draft: Result<Draft, CommandError>,
    pub(super) aligning: bool,
    pub(super) undo: Vec<(Options, SimplificationValues)>,
    pub(super) simplification: SimplificationValues,
}

impl Profiles {
    pub(super) fn capture_supports(
        &self,
        document: &Document,
    ) -> Arc<[Option<loft::TangentSupport>; 2]> {
        capture_supports(document, [self.sources.first(), self.sources.last()])
    }

    pub(super) fn from_seams(
        editor: &Editor,
        oriented: bool,
        tolerance: Tolerance,
    ) -> Result<Self, CommandError> {
        Ok(Self {
            sources: editor.sources().cloned().collect(),
            curves: editor.profiles(tolerance)?,
            oriented,
        })
    }

    pub(super) fn build(
        &self,
        options: Options,
        crease: bool,
        tolerance: Tolerance,
        supports: &[Option<loft::TangentSupport>; 2],
    ) -> Result<Draft, CommandError> {
        let brep = loft::prepare_with_tangent_supports(
            LoftProfiles::Aligned {
                curves: &self.curves,
                oriented: self.oriented,
            },
            options,
            crease,
            tolerance,
            [supports[0].as_ref(), supports[1].as_ref()],
        )?;
        Ok(Draft::new(brep, tolerance))
    }
}

impl Draft {
    pub(super) fn new(brep: Brep, tolerance: Tolerance) -> Self {
        let wires = draft_worker::wireframe(&brep, tolerance);
        Self { brep, wires }
    }
}

impl Seams {
    pub(super) fn aligned_profiles(&self, tolerance: Tolerance) -> Result<Profiles, CommandError> {
        Profiles::from_seams(&self.editor, self.oriented, tolerance)
    }
}

fn capture_supports(
    document: &Document,
    sources: [Option<&CapturedCurve>; 2],
) -> Arc<[Option<loft::TangentSupport>; 2]> {
    Arc::new(sources.map(|source| {
        let (id, edge) = section_key(source?);
        loft::TangentSupport::capture(document, id, edge?, None).ok()
    }))
}

impl Review {
    pub(super) fn tangent_sides(&self) -> [bool; 2] {
        [self.supports[0].is_some(), self.supports[1].is_some()]
    }

    pub(super) fn alignment_markers(&self) -> Vec<Marker> {
        if !self.aligning {
            return Vec::new();
        }
        self.profiles
            .curves
            .iter()
            .enumerate()
            .flat_map(|(index, curve)| {
                let curve = CurveRef::NurbsCurve(curve);
                [true, false]
                    .into_iter()
                    .filter_map(move |start| Marker::endpoint(index, curve, start))
            })
            .collect()
    }

    pub(super) fn alignment_overlay<'a>(&self, markers: &'a [Marker]) -> Option<Overlay<'a>> {
        self.aligning.then_some(Overlay {
            markers,
            drag: None,
            active: None,
            flipping: true,
            connect: false,
        })
    }
}

impl VibocerosApp {
    pub(in crate::app) fn apply_loft_seam_event(&mut self, viewport: usize, event: Event) -> bool {
        let Some(prompt) = &mut self.loft_prompt else {
            return false;
        };
        if let Some(review) = &mut prompt.review {
            if review.aligning
                && let Event::Flip(index) = event
            {
                self.flip_loft_profile(index);
                return true;
            }
            return false;
        }
        let Some(seams) = &mut prompt.seams else {
            return false;
        };
        let changed = seams.editor.apply(viewport, event);
        if matches!(event, Event::Flip(_)) {
            self.log_loft();
        }
        changed
    }

    pub(super) fn prepare_loft_sections(&self) -> Result<Sections, CommandError> {
        let prompt = self.loft_prompt.as_ref().unwrap();
        let tolerance = self.document.tolerance();
        let curves = prompt
            .state
            .sources
            .iter()
            .map(|section| section.source.curve().clone())
            .collect::<Vec<_>>();
        let closed = curves
            .iter()
            .map(NurbsCurve::is_closed)
            .collect::<Result<Vec<_>, _>>()?;
        if closed.iter().any(|closed| *closed) && closed.iter().any(|closed| !*closed) {
            return Err(CommandError::Usage(
                "Select either open or closed Loft sections",
            ));
        }
        let (automatic_order, oriented) = Brep::loft_profile_order(&curves, tolerance)?;
        // Edge matches refer to the first and last picked profiles.
        let order = if prompt.edge_highlights().next().is_some() {
            (0..curves.len()).collect()
        } else {
            automatic_order
        };
        let sources = order
            .iter()
            .map(|&index| prompt.state.sources[index].source.clone())
            .collect::<Vec<_>>();
        if closed.iter().all(|closed| *closed) {
            let supports = capture_supports(&self.document, [sources.first(), sources.last()]);
            let editor = Editor::from_sources(
                sources,
                tolerance,
                self.loft_snap_to_knots,
                prompt.state.options.natural_seams,
            );
            return Ok(Sections::Closed(Seams {
                editor,
                oriented,
                supports,
                preview: Default::default(),
            }));
        }
        let mut curves = order
            .iter()
            .map(|&index| {
                let curve = &curves[index];
                if prompt.state.sources[index].reversed == Some(true) {
                    curve.reversed()
                } else {
                    Ok(curve.clone())
                }
            })
            .collect::<Result<Vec<_>, _>>()?;
        let fixed = order
            .iter()
            .map(|&index| prompt.state.sources[index].reversed.is_some())
            .collect::<Vec<_>>();
        Brep::align_loft_open_profiles(&mut curves, &fixed)?;
        Ok(Sections::Open(Profiles {
            sources,
            curves,
            oriented,
        }))
    }

    pub(super) fn begin_loft_sections(&mut self) {
        match self.prepare_loft_sections() {
            Ok(Sections::Closed(seams)) => {
                self.loft_prompt.as_mut().unwrap().seams = Some(seams);
                self.log_loft();
            }
            Ok(Sections::Open(profiles)) => {
                let supports = profiles.capture_supports(&self.document);
                self.begin_loft_preview(profiles, supports, None);
            }
            Err(error) => self.push_log(format!("Unable to prepare Loft sections: {error}")),
        }
    }

    pub(super) fn accept_loft_seams(&mut self) {
        let prompt = self.loft_prompt.as_mut().unwrap();
        let seams = prompt.seams.as_mut().unwrap();
        let supports = seams.supports.clone();
        let prepared = if let Some((profiles, draft)) =
            seams.take_preview(prompt.state.options, self.commands.crease_splitting())
        {
            Ok((profiles, Some(draft)))
        } else {
            seams
                .aligned_profiles(self.document.tolerance())
                .map(|profiles| (profiles, None))
        };
        match prepared {
            Ok((profiles, draft)) => self.begin_loft_preview(profiles, supports, draft),
            Err(error) => self.push_log(format!("Unable to prepare Loft seams: {error}")),
        }
    }

    fn begin_loft_preview(
        &mut self,
        profiles: Profiles,
        supports: Arc<[Option<loft::TangentSupport>; 2]>,
        preview: Option<Draft>,
    ) {
        let prompt = self.loft_prompt.as_mut().unwrap();
        let draft = preview.map_or_else(
            || {
                profiles.build(
                    prompt.state.options,
                    self.commands.crease_splitting(),
                    self.document.tolerance(),
                    &supports,
                )
            },
            Ok,
        );
        prompt.review = Some(Review {
            profiles,
            supports,
            draft,
            aligning: false,
            undo: Vec::new(),
            simplification: self.loft_simplification.with_active(prompt.state.options),
        });
        self.report_loft_preview();
    }

    pub(super) fn refresh_loft_preview(&mut self) {
        let prompt = self.loft_prompt.as_mut().unwrap();
        if let Some(review) = &mut prompt.review {
            review.draft = review.profiles.build(
                prompt.state.options,
                self.commands.crease_splitting(),
                self.document.tolerance(),
                &review.supports,
            );
        }
        self.report_loft_preview();
    }

    fn report_loft_preview(&mut self) {
        if let Some(Err(error)) = self
            .loft_prompt
            .as_ref()
            .and_then(|prompt| prompt.review.as_ref())
            .map(|review| &review.draft)
        {
            self.push_log(format!("Unable to preview Loft: {error}"));
        }
        self.log_loft();
    }

    pub(super) fn align_loft_curves(&mut self) {
        let prompt = self.loft_prompt.as_mut().unwrap();
        let Some(review) = &mut prompt.review else {
            return;
        };
        if prompt.seams.is_some() {
            prompt.review = None;
        } else {
            review.aligning = true;
        }
        self.log_loft();
    }

    pub(super) fn flip_loft_source(&mut self, key: SectionKey) {
        let review = self.loft_prompt.as_ref().unwrap().review.as_ref().unwrap();
        if let Some(index) = review
            .profiles
            .sources
            .iter()
            .position(|source| section_key(source) == key)
        {
            self.flip_loft_profile(index);
        }
    }

    fn flip_loft_profile(&mut self, index: usize) {
        let curve = &mut self
            .loft_prompt
            .as_mut()
            .unwrap()
            .review
            .as_mut()
            .unwrap()
            .profiles
            .curves[index];
        match curve.reversed() {
            Ok(reversed) => {
                *curve = reversed;
                self.refresh_loft_preview();
            }
            Err(error) => self.push_log(format!("Unable to reverse Loft section: {error}")),
        }
    }

    pub(super) fn accept_loft_preview(&mut self) {
        let prompt = self.loft_prompt.as_mut().unwrap();
        let review = prompt.review.as_mut().unwrap();
        if review.aligning {
            review.aligning = false;
            self.log_loft();
            return;
        }
        if review.draft.is_err() {
            self.report_loft_preview();
            return;
        }

        let mut prompt = self.loft_prompt.take().unwrap();
        let review = prompt.review.take().unwrap();
        let draft = review.draft.expect("valid Loft preview");
        let faces = draft.brep.faces().len();
        let count = review.profiles.curves.len();
        match edit_document_transaction(&mut self.document, "Loft", |document| {
            document.add_geometry(Geometry::Brep(draft.brep))
        }) {
            Ok(_) => {
                self.restore_curve_selection(prompt.before);
                self.push_log(format!(
                    "Lofted {count} curve sections into one BRep with {faces} faces"
                ));
            }
            Err(error) => {
                prompt.review = Some(Review {
                    draft: Err(error.into()),
                    ..review
                });
                self.loft_prompt = Some(prompt);
                self.report_loft_preview();
            }
        }
    }
}
