//! Change a face's UV chart without changing its oriented spatial boundary.
use super::*;

#[derive(Clone, Copy)]
enum Edit {
    ReverseU,
    ReverseV,
    Swap,
}

impl Edit {
    fn point(self, p: Point2) -> Result<Point2, GeometryError> {
        match self {
            Self::ReverseU => Point2::try_new(-p.x(), p.y()),
            Self::ReverseV => Point2::try_new(p.x(), -p.y()),
            Self::Swap => Point2::try_new(p.y(), p.x()),
        }
    }

    fn iso(self, iso: SurfaceIso) -> SurfaceIso {
        use SurfaceIso::*;
        match (self, iso) {
            (Self::ReverseU, West) => East,
            (Self::ReverseU, East) => West,
            (Self::ReverseV, South) => North,
            (Self::ReverseV, North) => South,
            (Self::Swap, InteriorUConstant) => InteriorVConstant,
            (Self::Swap, InteriorVConstant) => InteriorUConstant,
            (Self::Swap, South) => West,
            (Self::Swap, East) => North,
            (Self::Swap, North) => East,
            (Self::Swap, West) => South,
            (_, iso) => iso,
        }
    }
}

impl Brep {
    /// Exchange selected face charts without moving spatial geometry or
    /// changing oriented incidence. Validate all indices and construct every
    /// replacement before committing; errors leave the B-rep untouched.
    /// This exact change preserves validity by construction and retains the
    /// spatial tables without recomputing their boundary certificates.
    pub fn try_swap_face_uv(&mut self, indices: &[usize]) -> Result<(), GeometryError> {
        self.edit_face_charts(indices, Edit::Swap)
    }

    /// Reverse selected U charts, compensating face sense and trim traversal
    /// so spatial geometry and oriented incidence remain unchanged. Native
    /// domains `[a,b]` become `[-b,-a]`. Invalid or repeated indices are atomic.
    pub fn try_reverse_face_u(&mut self, indices: &[usize]) -> Result<(), GeometryError> {
        self.edit_face_charts(indices, Edit::ReverseU)
    }

    /// Reverse selected V charts without changing oriented spatial geometry.
    /// See [`Self::try_reverse_face_u`] for domain and failure conventions.
    pub fn try_reverse_face_v(&mut self, indices: &[usize]) -> Result<(), GeometryError> {
        self.edit_face_charts(indices, Edit::ReverseV)
    }

    fn edit_face_charts(&mut self, indices: &[usize], edit: Edit) -> Result<(), GeometryError> {
        if indices.is_empty() {
            return Ok(());
        }
        let mut seen = vec![false; self.faces.len()];
        for &index in indices {
            let slot = seen
                .get_mut(index)
                .ok_or(GeometryError::InvalidBrepTopology {
                    context: "UV edit face index is out of range",
                })?;
            if std::mem::replace(slot, true) {
                return Err(GeometryError::InvalidBrepTopology {
                    context: "UV edit face indices must be unique",
                });
            }
        }
        let replacements = indices
            .iter()
            .map(|&i| Ok((i, self.faces[i].edit_chart(edit)?)))
            .collect::<Result<Vec<_>, GeometryError>>()?;
        self.extrusion = None;
        for (index, face) in replacements {
            self.faces[index] = face;
        }
        Ok(())
    }
}

impl BrepFace {
    /// Exchange the surface's U/V axes and compensate the face sense.
    /// Loop traversal, edge-use direction, UV tolerances and iso labels change
    /// with the chart; spatial edges and vertices require no changes.
    pub fn try_swapped_uv(&self) -> Result<Self, GeometryError> {
        self.edit_chart(Edit::Swap)
    }

    /// Reverse U and compensate face sense, loop traversal and edge uses.
    /// The surface and parameter-curve U coordinates are negated together.
    pub fn try_reversed_u(&self) -> Result<Self, GeometryError> {
        self.edit_chart(Edit::ReverseU)
    }

    /// Reverse V while retaining the face's oriented spatial boundary.
    pub fn try_reversed_v(&self) -> Result<Self, GeometryError> {
        self.edit_chart(Edit::ReverseV)
    }

    fn edit_chart(&self, edit: Edit) -> Result<Self, GeometryError> {
        let loops = self
            .loops
            .iter()
            .map(|boundary| {
                let trims = boundary
                    .trims
                    .iter()
                    .rev()
                    .map(|trim| {
                        let controls = trim
                            .curve
                            .control_points()
                            .iter()
                            .map(|c| WeightedPoint2::try_new(edit.point(c.point())?, c.weight()))
                            .collect::<Result<Vec<_>, GeometryError>>()?;
                        let curve = NurbsCurve2::try_new_rational(
                            trim.curve.degree(),
                            controls,
                            trim.curve.knots().to_vec(),
                        )?
                        .reversed()?;
                        Ok(BrepTrim {
                            vertices: [trim.vertices[1], trim.vertices[0]],
                            edge: trim.edge,
                            reversed_3d: trim.edge.is_some() && !trim.reversed_3d,
                            curve,
                            trim_type: trim.trim_type,
                            iso: edit.iso(trim.iso),
                            tolerance: match edit {
                                Edit::Swap => [trim.tolerance[1], trim.tolerance[0]],
                                _ => trim.tolerance,
                            },
                        })
                    })
                    .collect::<Result<Vec<_>, GeometryError>>()?;
                Ok(BrepLoop {
                    loop_type: boundary.loop_type,
                    trims,
                })
            })
            .collect::<Result<Vec<_>, GeometryError>>()?;
        let surface = match edit {
            Edit::Swap => self.surface.try_swapped_uv()?,
            Edit::ReverseU => self.surface.try_reversed_u()?,
            Edit::ReverseV => self.surface.try_reversed_v()?,
        };
        Self::try_new(surface, !self.reversed, loops)
    }
}

#[cfg(test)]
mod tests;
