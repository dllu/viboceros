//! Per-face meshing, boundary provenance audits, and conforming reconstruction.
mod planar_mesh;

use super::*;

mod audit;
mod conforming;
mod creases;
mod independent;
mod normals;

#[cfg(test)]
mod tests;
