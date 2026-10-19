//! Rational control-weight editing stages every owner before one history commit.
use super::*;

impl Geometry {
    pub fn supports_grip_weights(&self) -> bool {
        self.curve_ref().is_some()
            || matches!(self, Self::NurbsSurface(_))
            || matches!(self,Self::Brep(b)if b.faces().len()==1)
    }
    pub fn grip_weights(&self) -> Option<Vec<f64>> {
        match self {
            Self::PolyCurve(c) => c.grip_weights().ok(),
            Self::NurbsSurface(s) => Some(surface_weights(s)),
            Self::Brep(b) if b.faces().len() == 1 => Some(surface_weights(b.faces()[0].surface())),
            _ => {
                let c = self.curve_ref()?.to_nurbs().ok()?;
                Some(
                    c.control_points()
                        .iter()
                        .take(c.grip_count().ok()?)
                        .map(|c| c.weight())
                        .collect(),
                )
            }
        }
    }
    /// The displayed control-weight value. Composite grips expose their shared
    /// controller weight; their individual leaf weights remain in the geometry.
    pub fn grip_weight(&self, index: usize) -> Option<f64> {
        let surface_weight = |surface: &viboceros_geometry::NurbsSurface| {
            let [u, v] = surface.grip_dimensions();
            (index < u * v).then(|| {
                surface
                    .control_point(index / v, index % v)
                    .unwrap()
                    .weight()
            })
        };
        match self {
            Self::NurbsCurve(c) => {
                (index < c.grip_count().ok()?).then(|| c.control_points()[index].weight())
            }
            Self::PolyCurve(c) => c.grip_weights().ok()?.get(index).copied(),
            Self::NurbsSurface(s) => surface_weight(s),
            Self::Brep(b) if b.faces().len() == 1 => surface_weight(b.faces()[0].surface()),
            _ => {
                let c = self.curve_ref()?.to_nurbs().ok()?;
                (index < c.grip_count().ok()?).then(|| c.control_points()[index].weight())
            }
        }
    }

    /// Set absolute weights without moving Euclidean controls or changing the
    /// curve/surface basis. The source remains intact if reconstruction fails.
    pub fn with_grip_weight(
        &self,
        indices: &BTreeSet<usize>,
        weight: f64,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        if !weight.is_finite() || weight == 0. {
            return Err(GeometryError::InvalidWeight { index: 0 });
        }
        if self
            .grip_weights()
            .is_some_and(|weights| indices.iter().all(|&i| weights.get(i) == Some(&weight)))
        {
            return Ok(self.clone());
        }
        Ok(match self {
            Self::PolyCurve(c) => Self::PolyCurve(c.with_grip_weight(indices, weight)?),
            Self::NurbsSurface(s) => Self::NurbsSurface(s.with_grip_weight(indices, weight)?),
            Self::Brep(b) if b.faces().len() == 1 => {
                let old = b.faces()[0].surface();
                let next = old.with_grip_weight(indices, weight)?;
                if next == *old {
                    return Ok(self.clone());
                }
                Self::Brep(b.try_with_edited_single_surface(next, tolerance)?)
            }
            _ if self.curve_ref().is_some() => {
                let c = self
                    .curve_ref()
                    .unwrap()
                    .to_nurbs()?
                    .with_grip_weight(indices, weight)?;
                let cuts = if c.degree() > 1 {
                    c.full_multiplicity_c1_break_parameters()?
                } else {
                    Vec::new()
                };
                if !cuts.is_empty() {
                    let mut parameters = vec![*c.domain().start()];
                    parameters.extend(cuts.iter().copied());
                    parameters.push(*c.domain().end());
                    let mut remainder = c;
                    let mut pieces = Vec::new();
                    for cut in cuts {
                        let (left, right) = remainder.try_split(cut)?;
                        pieces.push(left);
                        remainder = right;
                    }
                    pieces.push(remainder);
                    Self::PolyCurve(viboceros_geometry::PolyCurve3::try_with_segment_domains(
                        pieces, parameters,
                    )?)
                } else if c.degree() == 1
                    && !c.is_rational()
                    && c.knots()[1..c.knots().len() - 1]
                        .windows(2)
                        .all(|k| k[0] < k[1])
                {
                    Self::Polyline(viboceros_geometry::Polyline3::try_with_parameters(
                        c.control_points().iter().map(|p| p.point()).collect(),
                        c.knots()[1..c.knots().len() - 1].to_vec(),
                        tolerance,
                    )?)
                } else {
                    Self::NurbsCurve(c)
                }
            }
            _ => {
                return Err(GeometryError::UnsupportedControlPointEdit {
                    context: "weights require curve or surface control points",
                });
            }
        })
    }
}

impl Document {
    pub fn control_point_weights(&self, object: ObjectId) -> Option<Vec<f64>> {
        (self.grip_mode(object) == Some(GripMode::Control))
            .then(|| self.object(object)?.geometry().grip_weights())
            .flatten()
    }

    /// Enumerate eligible selected controls with one weight-net read per owner.
    pub fn selected_weight_control_points(&self) -> impl Iterator<Item = (ControlPointId, f64)> {
        let mut nets = BTreeMap::new();
        let mut result = Vec::new();
        for (p, _) in self.selected_control_points() {
            let net = nets
                .entry(p.object)
                .or_insert_with(|| self.control_point_weights(p.object));
            if let Some(weight) = net.as_ref().and_then(|w| w.get(p.index)) {
                result.push((p, *weight));
            }
        }
        result.into_iter()
    }
    pub fn control_point_weight(&self, point: ControlPointId) -> Option<f64> {
        let state = self.control_points.get(&point.object)?;
        if state.mode != GripMode::Control || point.index >= state.points.len() {
            return None;
        }
        let geometry = self.object(point.object)?.geometry();
        geometry.grip_weight(point.index)
    }

    /// Set the weight of explicit, selectable control points. Duplicate picks
    /// coalesce, and every source is validated before geometry or history changes.
    pub fn set_control_point_weights(
        &mut self,
        points: impl IntoIterator<Item = ControlPointId>,
        weight: f64,
    ) -> Result<usize, DocumentError> {
        if !weight.is_finite() || weight <= 0. {
            return Err(GeometryError::InvalidWeight { index: 0 }.into());
        }
        let mut picks = BTreeMap::<ObjectId, BTreeSet<usize>>::new();
        for p in points {
            if !self.is_control_point_selectable(p)
                || self.grip_mode(p.object) != Some(GripMode::Control)
                || !self
                    .object(p.object)
                    .is_some_and(|o| o.geometry().supports_grip_weights())
            {
                return Err(DocumentError::InvalidControlPointSelection {
                    object: p.object,
                    index: p.index,
                });
            }
            picks.entry(p.object).or_default().insert(p.index);
        }
        if picks.is_empty() {
            return Ok(0);
        }
        picks.retain(|id, indices| {
            self.control_point_weights(*id)
                .is_some_and(|w| indices.iter().any(|&i| w[i] != weight))
        });
        if picks.is_empty() {
            return Ok(0);
        }
        let indices = self.resolve_object_indices(picks.keys().copied())?;
        for &i in &indices {
            self.ensure_object_editable(&self.objects[i])?;
        }
        let before = indices
            .iter()
            .map(|&i| {
                let id = self.objects[i].id;
                (id, self.control_points[&id].clone())
            })
            .collect::<Vec<_>>();
        let staged = indices
            .iter()
            .map(|&i| {
                Ok((
                    i,
                    self.objects[i].geometry.with_grip_weight(
                        &picks[&self.objects[i].id],
                        weight,
                        self.tolerance,
                    )?,
                ))
            })
            .collect::<Result<Vec<_>, DocumentError>>()?;
        let owners = indices
            .iter()
            .map(|&i| self.objects[i].id)
            .collect::<Vec<_>>();
        let owns = self.history.active.is_none();
        if owns {
            self.begin_transaction("Weight")?;
        }
        let result = (|| {
            let count = self.commit_object_geometries(
                staged,
                "Weight",
                "Weight",
                ReplacementHistory::EveryReplacement,
                true,
            )?;
            for (id, original) in before {
                let current = self.control_points.get_mut(&id).unwrap();
                current.selected = Arc::new(
                    original
                        .selected
                        .iter()
                        .copied()
                        .filter(|i| !current.hidden.contains(i) || picks[&id].contains(i))
                        .collect(),
                );
                self.record_edit(
                    "Weight",
                    Edit::ControlPointWeightsChanged {
                        id,
                        stored: Some(original),
                    },
                );
            }
            self.move_objects_to_end_in_order(owners)?;
            Ok(count)
        })();
        if owns {
            if result.is_ok() {
                self.commit_transaction()?;
            } else {
                self.rollback_transaction()?;
            }
        }
        result
    }
}

fn surface_weights(s: &viboceros_geometry::NurbsSurface) -> Vec<f64> {
    let [u, v] = s.grip_dimensions();
    (0..u)
        .flat_map(|i| (0..v).map(move |j| s.control_point(i, j).unwrap().weight()))
        .collect()
}
