//! Only curve endpoints own source objects; free point endpoints own no geometry.
use super::*;
impl Input {
    pub fn commit(&self, doc: &mut Document) -> Result<ObjectId, CommandError> {
        if !self.ready() || !self.current(doc) {
            return Err(CommandError::Usage(
                "BlendCrv references changed; pick the curves again",
            ));
        }
        let curve = self.preview.as_ref().ok_or(CommandError::Usage(USAGE))?;
        let guides = self
            .endpoints
            .iter()
            .filter_map(Endpoint::curve)
            .collect::<Vec<_>>();
        if (self.preferences.trim || self.preferences.join)
            && guides
                .iter()
                .any(|g| g.source.source().1.is_some() && !g.temporary())
        {
            return Err(CommandError::Usage(
                "BlendCrv surface edges currently require Trim=No and Join=No",
            ));
        }
        if self.preferences.join
            && guides.len() == 2
            && guides.iter().all(|g| !g.temporary())
            && guides[0].source.object_id() == guides[1].source.object_id()
        {
            return Err(CommandError::Usage(
                "BlendCrv joined sources must be distinct",
            ));
        }
        let mut replacements = vec![];
        let mut pieces = vec![];
        let mut consumed = vec![];
        let mut inherited_source = None;
        if self.preferences.trim || self.preferences.join {
            for (i, endpoint) in self.endpoints.iter().enumerate() {
                if i == 1 {
                    pieces.push(
                        viboceros_geometry::Curve3::NurbsCurve(curve.clone()).to_polycurve()?,
                    );
                }
                let Some(g) = endpoint.curve() else {
                    continue;
                };
                // Unjoined private portions never replace their original parent.
                // In PickPoint mode the retained side may even be empty.
                if g.temporary() && !self.preferences.join {
                    continue;
                }
                let c = g.curve_ref();
                let retained_end = g.end ^ g.flip;
                let domain = if !self.preferences.trim {
                    c.domain()
                } else if retained_end {
                    *c.domain().start()..=g.parameter
                } else {
                    g.parameter..=*c.domain().end()
                };
                let changed = domain != c.domain();
                let source = c.to_owned();
                let piece = if changed {
                    crate::curve_segments::trimmed(&source, domain)?
                } else {
                    source
                };
                if changed && self.preferences.trim && !g.temporary() {
                    replacements.push((g.source.object_id(), Geometry::from(piece.clone())));
                }
                let reverse = if i == 0 { !retained_end } else { retained_end };
                let piece = if reverse {
                    piece.reversed(self.tolerance)?
                } else {
                    piece
                };
                if self.preferences.join {
                    let (source_end, blend_end) = if i == 0 {
                        (
                            piece.as_ref().end_point()?,
                            curve.evaluate(*curve.domain().start())?,
                        )
                    } else {
                        (
                            piece.as_ref().start_point()?,
                            curve.evaluate(*curve.domain().end())?,
                        )
                    };
                    if source_end.distance_to(blend_end)? > self.tolerance.absolute() {
                        // Native partial joins leave disconnected whole sources
                        // untouched. A blend alone still becomes a PolyCurve.
                        if !self.preferences.trim {
                            continue;
                        }
                        return Err(CommandError::Usage(
                            "The retained source does not meet this blend contact",
                        ));
                    }
                    if !g.temporary() {
                        consumed.push(g.source.object_id());
                        inherited_source.get_or_insert(g.source.object_id());
                    }
                }
                pieces.push(piece.to_polycurve()?);
            }
        }
        let output = self
            .preferences
            .join
            .then(|| viboceros_geometry::PolyCurve3::concatenate(&pieces))
            .transpose()?;
        let selected = doc.selected_object_ids().collect::<Vec<_>>();
        doc.release_command_selection_on_history_replay(selected)?;
        doc.clear_selection();
        if let Some(output) = output {
            if let Some(first) = inherited_source {
                doc.replace_object_geometries([(first, Geometry::PolyCurve(output))])?;
                doc.delete_objects(consumed.into_iter().filter(|id| *id != first))?;
                Ok(first)
            } else {
                let id = doc.add_geometry(Geometry::PolyCurve(output))?;
                doc.delete_objects(consumed)?;
                Ok(id)
            }
        } else {
            if self.preferences.trim {
                doc.replace_object_geometries(replacements)?;
            }
            Ok(doc.add_geometry(Geometry::NurbsCurve(curve.clone()))?)
        }
    }
}
