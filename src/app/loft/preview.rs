//! A completed draft is displayed and then moved into the document on acceptance.
use super::*;
use crate::viewport::curve_seam::Overlay;
use viboceros_command::loft;
use viboceros_geometry::{Brep, CurveRef, LoftProfiles, NurbsCurve};

#[derive(Debug)]
pub(super) struct Profiles {
    sources: Vec<CapturedCurve>,
    curves: Vec<NurbsCurve>,
    oriented: bool,
}

#[derive(Debug)]
pub(super) struct Draft {
    brep: Brep,
    pub(super) wires: Vec<NurbsCurve>,
}

#[derive(Debug)]
pub(super) struct Review {
    profiles: Profiles,
    pub(super) draft: Result<Draft, CommandError>,
    pub(super) aligning: bool,
    pub(super) undo: Vec<(Options, SimplificationValues)>,
    pub(super) simplification: SimplificationValues,
}

impl Profiles {
    fn build(
        &self,
        options: Options,
        crease: bool,
        tolerance: Tolerance,
    ) -> Result<Draft, CommandError> {
        let brep = loft::prepare(
            LoftProfiles::Aligned {
                curves: &self.curves,
                oriented: self.oriented,
            },
            options,
            crease,
            tolerance,
        )?;
        let wires = brep.wireframe_curves(1, tolerance).unwrap_or_else(|_| {
            brep.edges()
                .iter()
                .map(|edge| edge.curve().clone())
                .collect()
        });
        Ok(Draft { brep, wires })
    }
}

impl Review {
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

    pub(super) fn begin_loft_sections(&mut self) {
        let Some(prompt) = &self.loft_prompt else {
            return;
        };
        let tolerance = self.document.tolerance();
        let prepared = (|| {
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
            let (order, oriented) = Brep::loft_profile_order(&curves, tolerance)?;
            let sources = order
                .iter()
                .map(|&index| prompt.state.sources[index].source.clone())
                .collect::<Vec<_>>();
            if closed.iter().all(|closed| *closed) {
                let editor = Editor::from_sources(
                    sources,
                    tolerance,
                    self.loft_snap_to_knots,
                    prompt.state.options.natural_seams,
                );
                return Ok((Some(Seams { editor, oriented }), None));
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
            Ok((
                None,
                Some(Profiles {
                    sources,
                    curves,
                    oriented,
                }),
            ))
        })();
        match prepared {
            Ok((Some(seams), _)) => {
                self.loft_prompt.as_mut().unwrap().seams = Some(seams);
                self.log_loft();
            }
            Ok((_, Some(profiles))) => self.begin_loft_preview(profiles),
            Ok(_) => unreachable!("Loft sections produce an editor or preview"),
            Err(error) => self.push_log(format!("Unable to prepare Loft sections: {error}")),
        }
    }

    pub(super) fn accept_loft_seams(&mut self) {
        let seams = self.loft_prompt.as_ref().unwrap().seams.as_ref().unwrap();
        match seams.editor.profiles(self.document.tolerance()) {
            Ok(curves) => {
                let sources = seams.editor.sources().cloned().collect();
                self.begin_loft_preview(Profiles {
                    curves,
                    sources,
                    oriented: seams.oriented,
                });
            }
            Err(error) => self.push_log(format!("Unable to prepare Loft seams: {error}")),
        }
    }

    fn begin_loft_preview(&mut self, profiles: Profiles) {
        let prompt = self.loft_prompt.as_mut().unwrap();
        let draft = profiles.build(
            prompt.state.options,
            self.commands.crease_splitting(),
            self.document.tolerance(),
        );
        prompt.review = Some(Review {
            profiles,
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
