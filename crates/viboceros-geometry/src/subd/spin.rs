//! Control-edge spins preserve positions and native component identities.
use super::*;
impl SubD {
    pub fn try_spin_edge(
        &self,
        index: usize,
        clockwise: bool,
    ) -> Result<(Self, bool), GeometryError> {
        if index >= self.edges().len() {
            return Err(viboceros_opennurbs::SubDError(
                "SubD spin edge outside its source table".into(),
            )
            .into());
        }
        let (data, changed) =
            viboceros_opennurbs::subd_spin_edge(self.archive(), index, clockwise)?;
        if changed {
            Self::from_native(data).map(|s| (s, true))
        } else {
            Ok((self.clone(), false))
        }
    }
}
