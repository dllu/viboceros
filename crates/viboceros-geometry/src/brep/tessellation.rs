//! Per-face meshing, boundary provenance audits, and conforming reconstruction.

use super::*;

mod audit;
mod conforming;
mod independent;
mod normals;

#[cfg(test)]
mod tests;
