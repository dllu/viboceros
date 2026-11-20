use super::*;
use crate::viewport::curve_seam::{Marker, Overlay};
use std::sync::Arc;
use viboceros_geometry::CurveRef;

#[derive(Debug)]
pub(super) struct Alignment {
    profiles: Vec<Profile>,
    undo_from: usize,
}

#[derive(Debug)]
struct Profile {
    id: ObjectId,
    // End markers in the original and reversed traversal directions.
    markers: [[Option<Marker>; 2]; 2],
}

impl Session {
    pub(in crate::app) fn alignment_markers(&self) -> Option<Vec<Marker>> {
        Some(
            self.alignment
                .as_ref()?
                .profiles
                .iter()
                .flat_map(|profile| {
                    let reversed = self.state.reversed_profiles.contains(&profile.id);
                    profile.markers[usize::from(reversed)].into_iter().flatten()
                })
                .collect(),
        )
    }

    pub(in crate::app) fn alignment_overlay<'a>(
        &self,
        markers: &'a [Marker],
    ) -> Option<Overlay<'a>> {
        self.alignment
            .as_ref()
            .map(|_| Overlay::directions(markers))
    }
}

impl VibocerosApp {
    pub(super) fn begin_sweep_alignment(&mut self) {
        if !self.validate_sweep_sources() {
            return;
        }
        let p = self.sweep_session.as_mut().unwrap();
        let tolerance = self.document.tolerance();
        let profiles = p.state.capture_source(&self.document).and_then(|source| {
            let source =
                seams::with_seam_profiles(Arc::new(source), p.state.stage.seams(), tolerance)?;
            source
                .profile_curves()
                .enumerate()
                .map(|(index, profile)| {
                    let (id, curve) = profile?;
                    let closed = curve.is_closed()?;
                    let curve = CurveRef::NurbsCurve(&curve);
                    let markers = [false, true].map(|reversed| {
                        [
                            Marker::endpoint(index, curve, true, reversed),
                            (!closed)
                                .then(|| Marker::endpoint(index, curve, false, reversed))
                                .flatten(),
                        ]
                    });
                    Ok(Profile { id, markers })
                })
                .collect::<Result<Vec<_>, CommandError>>()
        });
        match profiles {
            Ok(profiles) => {
                p.alignment = Some(Alignment {
                    profiles,
                    undo_from: p.undo.len(),
                });
                self.log_sweep_input();
            }
            Err(error) => self.push_log(format!("Unable to read Sweep1 cross sections: {error}")),
        }
    }

    pub(super) fn continue_sweep_alignment(&mut self, input: &str) {
        let p = self.sweep_session.as_mut().unwrap();
        if input.is_empty() || input.eq_ignore_ascii_case("Enter") {
            if !std::mem::take(&mut p.seam_id_pending) {
                self.finish_sweep_alignment();
                return;
            }
        } else {
            let (name, value) = input
                .split_once(|c: char| c == '=' || c.is_whitespace())
                .map_or((input, ""), |(name, value)| (name, value.trim()));
            let id = if p.seam_id_pending || !name.eq_ignore_ascii_case("SelID") {
                input
            } else {
                value
            };
            p.seam_id_pending = id.is_empty();
            if let Ok(id) = id.parse::<ObjectId>()
                && let Some(index) = p
                    .alignment
                    .as_ref()
                    .unwrap()
                    .profiles
                    .iter()
                    .position(|profile| profile.id == id)
            {
                self.flip_sweep_section(index);
                return;
            }
        }
        self.log_sweep_input();
    }

    pub(super) fn flip_sweep_section(&mut self, index: usize) -> bool {
        if !self.validate_sweep_sources() {
            return false;
        }
        let p = self.sweep_session.as_mut().unwrap();
        let Some(profile) = p
            .alignment
            .as_ref()
            .and_then(|alignment| alignment.profiles.get(index))
        else {
            return false;
        };
        p.undo.push(p.state.clone());
        if !p.state.reversed_profiles.remove(&profile.id) {
            p.state.reversed_profiles.insert(profile.id);
        }
        p.seam_id_pending = false;
        self.log_sweep_input();
        true
    }

    pub(super) fn undo_sweep_alignment(&mut self) -> bool {
        let Some(p) = self.sweep_session.as_mut() else {
            return false;
        };
        let Some(alignment) = p.alignment.as_ref() else {
            return false;
        };
        if !std::mem::take(&mut p.seam_id_pending) {
            if p.undo.len() > alignment.undo_from {
                p.state = p.undo.pop().unwrap();
            } else {
                p.alignment = None;
            }
        }
        self.log_sweep_input();
        true
    }

    pub(in crate::app) fn finish_sweep_alignment(&mut self) -> bool {
        let Some(p) = self.sweep_session.as_mut() else {
            return false;
        };
        if p.alignment.take().is_none() {
            return false;
        }
        p.seam_id_pending = false;
        self.log_sweep_input();
        true
    }
}
