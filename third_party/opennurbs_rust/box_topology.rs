// Copyright (c) 1993-2022 Robert McNeel & Associates. All rights reserved.
// OpenNURBS, Rhinoceros, and Rhino3D are registered trademarks of Robert
// McNeel & Associates.
//
// THIS SOFTWARE IS PROVIDED "AS IS" WITHOUT EXPRESS OR IMPLIED WARRANTY.
// ALL IMPLIED WARRANTIES OF FITNESS FOR ANY PARTICULAR PURPOSE AND OF
// MERCHANTABILITY ARE HEREBY DISCLAIMED.
//
// Modified Rust adaptation of ON_BrepBox's connectivity tables and
// ON_NurbsSurfaceQuadrilateral/ON_LineCurve's parameter extent policy from
// the pinned openNURBS source. See README.md and LICENSE in this directory.

pub(crate) const CORNERS: [[usize; 3]; 8] = [
    [0, 0, 0],
    [1, 0, 0],
    [1, 1, 0],
    [0, 1, 0],
    [0, 0, 1],
    [1, 0, 1],
    [1, 1, 1],
    [0, 1, 1],
];
pub(crate) const EDGES: [[usize; 2]; 12] = [
    [0, 1],
    [1, 2],
    [2, 3],
    [3, 0],
    [4, 5],
    [5, 6],
    [6, 7],
    [7, 4],
    [0, 4],
    [1, 5],
    [2, 6],
    [3, 7],
];
pub(crate) const FACES: [([usize; 4], [bool; 4]); 6] = [
    ([0, 9, 4, 8], [false, false, true, true]),
    ([1, 10, 5, 9], [false, false, true, true]),
    ([2, 11, 6, 10], [false, false, true, true]),
    ([3, 8, 7, 11], [false, false, true, true]),
    ([3, 2, 1, 0], [true, true, true, true]),
    ([4, 5, 6, 7], [false, false, false, false]),
];
pub(crate) fn parameter_extent(first: f64, second: f64) -> f64 {
    let extent = first.max(second);
    if extent <= 2.328_306_436_538_696_3e-10 {
        1.
    } else {
        extent
    }
}
