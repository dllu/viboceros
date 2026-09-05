//! Per-face meshing, boundary provenance audits, and conforming reconstruction.

use super::*;

mod audit;
mod conforming;
mod independent;

#[cfg(test)]
mod tests;
