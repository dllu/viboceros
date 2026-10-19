//! Logical grip neighbors use parameter indices, independently of projection.
use super::*;
use viboceros_document::{ControlPointId, GripMode};
#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Navigation {
    NextU,
    PrevU,
    NextV,
    PrevV,
    AddNextU,
    AddPrevU,
    AddNextV,
    AddPrevV,
    SelU,
    SelV,
}

impl Navigation {
    pub fn accepts(geometry: &Geometry) -> bool {
        geometry.curve_ref().is_some()
            || matches!(geometry, Geometry::NurbsSurface(_))
            || matches!(geometry, Geometry::Brep(b) if b.faces().len() == 1)
    }
    pub const fn name(self) -> &'static str {
        match self {
            Self::NextU => "NextU",
            Self::PrevU => "PrevU",
            Self::NextV => "NextV",
            Self::PrevV => "PrevV",
            Self::AddNextU => "AddNextU",
            Self::AddPrevU => "AddPrevU",
            Self::AddNextV => "AddNextV",
            Self::AddPrevV => "AddPrevV",
            Self::SelU => "SelU",
            Self::SelV => "SelV",
        }
    }

    pub(super) fn apply(self, document: &mut Document) -> Result<String, CommandError> {
        let mut old = Vec::new();
        let mut next = BTreeSet::new();
        let mut owners = std::collections::BTreeMap::<ObjectId, Vec<usize>>::new();
        for (grip, _) in document.selected_control_points() {
            owners.entry(grip.object).or_default().push(grip.index);
        }
        for (object_id, seeds) in owners {
            let object = document.object(object_id).unwrap();
            let Some(layout) = Layout::of(document, object)? else {
                continue;
            };
            old.extend(seeds.iter().map(|&index| ControlPointId {
                object: object_id,
                index,
            }));
            for index in layout.select(self, &seeds) {
                next.insert(ControlPointId {
                    index,
                    object: object_id,
                });
            }
        }
        // Stage every index before touching selection. Object selections and
        // unrelated display state survive this non-history operation.
        document.select_control_points(old, SelectionMode::Remove)?;
        let count = document.select_control_points(next, SelectionMode::Add)?;
        Ok(format!(
            "Selected {} object(s), {count} grip(s)",
            document.selected_object_count()
        ))
    }
}

struct Layout {
    dimensions: [usize; 2],
    wrap: [bool; 2],
    curve: bool,
    logical: bool,
}
impl Layout {
    fn of(
        document: &Document,
        object: &viboceros_document::Object,
    ) -> Result<Option<Self>, CommandError> {
        if !Navigation::accepts(object.geometry()) {
            return Ok(None);
        }
        let count = document.control_point_locations(object.id()).unwrap().len();
        let unstructured = Self {
            dimensions: [count, 1],
            wrap: [false; 2],
            curve: false,
            logical: false,
        };
        if document.grip_mode(object.id()) == Some(GripMode::Edit) {
            return Ok(Some(unstructured));
        }
        let surface = match object.geometry() {
            Geometry::NurbsSurface(s) => Some(s),
            Geometry::Brep(b) if b.faces().len() == 1 => Some(b.faces()[0].surface()),
            _ => None,
        };
        let result = if let Some(surface) = surface {
            Self {
                dimensions: surface.grip_dimensions(),
                wrap: [surface.is_periodic_u(), surface.is_periodic_v()],
                curve: false,
                logical: true,
            }
        } else if let Some(curve) = object.geometry().curve_ref() {
            let wrap = match object.geometry() {
                Geometry::NurbsCurve(curve) => curve.is_periodic(),
                Geometry::Circle(_) | Geometry::Arc(_) => curve.is_closed()?,
                _ => curve.to_nurbs()?.is_periodic(),
            };
            Self {
                dimensions: [count, 1],
                wrap: [wrap, false],
                curve: true,
                logical: true,
            }
        } else {
            return Ok(Some(unstructured));
        };
        Ok((result.dimensions.iter().product::<usize>() == count).then_some(result))
    }

    fn select(&self, action: Navigation, seeds: &[usize]) -> Vec<usize> {
        use Navigation::*;
        if !self.logical {
            return if matches!(action, NextU | PrevU | NextV | PrevV) {
                Vec::new()
            } else {
                seeds.to_vec()
            };
        }
        let action = if self.curve && action == SelV {
            SelU
        } else {
            action
        };
        let (axis, positive, add) = match action {
            NextU => (0, true, false),
            PrevU => (0, false, false),
            NextV => (1, true, false),
            PrevV => (1, false, false),
            AddNextU => (0, true, true),
            AddPrevU => (0, false, true),
            AddNextV => (1, true, true),
            AddPrevV => (1, false, true),
            SelU | SelV => {
                let u = seeds
                    .iter()
                    .map(|i| i / self.dimensions[1])
                    .collect::<BTreeSet<_>>();
                let v = seeds
                    .iter()
                    .map(|i| i % self.dimensions[1])
                    .collect::<BTreeSet<_>>();
                return (0..self.dimensions[0] * self.dimensions[1])
                    .filter(|&i| {
                        (matches!(action, SelU) && v.contains(&(i % self.dimensions[1])))
                            || (matches!(action, SelV) && u.contains(&(i / self.dimensions[1])))
                    })
                    .collect();
            }
        };
        let axis = if self.curve { 0 } else { axis };
        let mut result = if add { seeds.to_vec() } else { Vec::new() };
        for &index in seeds {
            let mut uv = [index / self.dimensions[1], index % self.dimensions[1]];
            let k = uv[axis];
            let size = self.dimensions[axis];
            let neighbor = if positive {
                k.checked_add(1).filter(|&i| i < size)
            } else {
                k.checked_sub(1)
            }
            .or_else(|| self.wrap[axis].then_some(if positive { 0 } else { size - 1 }));
            if let Some(k) = neighbor {
                uv[axis] = k;
                result.push(uv[0] * self.dimensions[1] + uv[1]);
            }
        }
        result
    }
}
