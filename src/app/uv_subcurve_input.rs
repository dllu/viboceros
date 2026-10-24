//! Shared nonmutating SubCrv input for commands that collect curve references.
use super::*;
use viboceros_command::curve_reference::CapturedCurve;
use viboceros_document::ObjectId;
mod edges;
mod entry;
mod points;
mod state;
pub(super) use state::{CurveRange, PendingSubcurve, SubcurveInputs};
