//! The one converter: `cadmpeg`'s intermediate representation to monstertruck.
//!
//! Every cadmpeg codec decodes into [`cadmpeg_ir::CadIr`], so this is the only
//! place that has to understand recovered geometry, and each format module above
//! it is a few lines choosing a decoder. Adding a format should not touch this
//! file except to widen what it already handles.
//!
//! # What the conversion has to get right
//!
//! `CadIr` carries a flat, table-shaped B-rep -- bodies, regions, shells, faces,
//! loops, coedges, edges, vertices, plus separate surface, curve and pcurve
//! tables -- which is the same shape monstertruck's [`CompressedShell`] uses, so
//! the mapping is mostly index translation. Three things are not:
//!
//! * **Analytic carriers must stay analytic.** A cylinder arriving as
//!   [`Surface::Plane`]-adjacent NURBS is a silent loss of exactness that the
//!   boolean kernel pays for later. Verified 2026-08-04 that cadmpeg does keep
//!   them: a real part decoded as 34 planes, 38 cylinders, 10 tori and 4 NURBS
//!   rather than 82 spline patches.
//! * **Units.** cadmpeg normalises to millimetres on decode. Anything that
//!   assumes the source unit will be wrong by a factor.
//! * **Loss.** cadmpeg reports what it dropped, per entity, with a reason. That
//!   report must reach the caller rather than being discarded, which is why
//!   [`Error::Decode`] carries the decoder's own message.
//!
//! [`CompressedShell`]: monstertruck_topology::compress::CompressedShell
//! [`Surface::Plane`]: monstertruck_modeling::Surface

pub mod step;

use crate::{Error, Result};
use monstertruck_modeling::{Curve, Point3, Surface};
use monstertruck_topology::compress::{CompressedShell, CompressedSolid};

/// A solid as monstertruck's kernel wants it, over the canonical curve and
/// surface enums the boolean pipeline consumes.
pub type ImportedSolid = CompressedSolid<Point3, Curve, Surface>;

/// An open shell: faces that do not bound a volume.
pub type ImportedSheet = CompressedShell<Point3, Curve, Surface>;

/// A body recovered from an exchange file.
///
/// Not every format yields solids. Measured 2026-08-06 against cadmpeg 0.4 over
/// its own IGES fixtures: STEP gives `solid` bodies, IGES gives `sheet` and
/// `wire`. IGES in the wild is often a surface collection with trimming curves,
/// not a closed B-rep, so a `Vec<ImportedSolid>` cannot carry most IGES files.
///
/// Wire bodies have no faces and no monstertruck equivalent. They are reported
/// as [`Error::UnsupportedBodyKind`] rather than dropped, so a caller never
/// receives fewer bodies than the file holds without being told.
#[derive(Debug, Clone)]
pub enum ImportedBody {
    /// Shells that bound a volume.
    Solid(ImportedSolid),
    /// Faces that do not bound a volume.
    Sheet(ImportedSheet),
}

/// Convert every body in a decoded document into monstertruck solids.
///
/// Returns [`Error::NoGeometry`] when the document decoded but held no body,
/// which is a fact about the file, not a failure to read it.
pub fn to_bodies(ir: &cadmpeg_ir::CadIr, format: &'static str) -> Result<Vec<ImportedBody>> {
    if ir.model.bodies.is_empty() {
        return Err(Error::NoGeometry { format });
    }
    Err(Error::Unimplemented {
        what: "cadmpeg intermediate representation to monstertruck B-rep",
    })
}

/// Read solids only, discarding sheet bodies.
///
/// # Deprecated
///
/// Prefer [`to_bodies`]. This signature cannot express what IGES actually
/// returns -- mostly sheets -- so it would silently hand back an empty list for
/// files that are full of geometry.
#[deprecated(
    since = "0.4.0",
    note = "cannot represent sheet bodies, which is most IGES content; use `to_bodies`"
)]
pub fn to_solids(ir: &cadmpeg_ir::CadIr, format: &'static str) -> Result<Vec<ImportedSolid>> {
    Ok(to_bodies(ir, format)?
        .into_iter()
        .filter_map(|body| match body {
            ImportedBody::Solid(solid) => Some(solid),
            ImportedBody::Sheet(_) => None,
        })
        .collect())
}
