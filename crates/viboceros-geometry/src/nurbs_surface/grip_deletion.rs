//! A surface deletion removes complete rows in either or both parameter axes.
use super::*;
use std::collections::BTreeSet;

impl NurbsSurface {
    pub fn try_delete_grips(
        &self,
        indices: &BTreeSet<usize>,
    ) -> Result<Option<Self>, GeometryError> {
        let [u, v] = self.grip_dimensions();
        let count = u * v;
        if let Some(&index) = indices.last().filter(|&&i| i >= count) {
            return Err(GeometryError::InvalidControlPointIndex { index, count });
        }
        if indices.len() == count {
            return Ok(None);
        }
        let rows = (0..u)
            .filter(|i| (0..v).all(|j| indices.contains(&(i * v + j))))
            .collect::<BTreeSet<_>>();
        let columns = (0..v)
            .filter(|j| (0..u).all(|i| indices.contains(&(i * v + j))))
            .collect::<BTreeSet<_>>();
        if u - rows.len() < 2 || v - columns.len() < 2 {
            return Ok(Some(self.clone()));
        }
        let mut result = self.clone();
        if !rows.is_empty() {
            result = result.map_u_control_curves(|curve| {
                curve
                    .delete_grips(&rows, false)?
                    .ok_or(GeometryError::InvalidControlNet {
                        context: "surface deletion removed a whole control curve",
                    })
            })?;
        }
        if !columns.is_empty() {
            result = result.map_v_control_curves(|curve| {
                curve
                    .delete_grips(&columns, false)?
                    .ok_or(GeometryError::InvalidControlNet {
                        context: "surface deletion removed a whole control curve",
                    })
            })?;
        }
        Ok(Some(result))
    }
}
