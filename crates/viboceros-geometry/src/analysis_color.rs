//! Saturated OpenNURBS analysis palette, parameterized from red to blue.
// Rust adaptation of ON_Color::SetHSV; altered source.
// Portions Copyright (c) 1993-2022 Robert McNeel & Associates. All rights reserved.
// THIS SOFTWARE IS PROVIDED "AS IS" WITHOUT EXPRESS OR IMPLIED WARRANTY.
// ALL IMPLIED WARRANTIES OF FITNESS FOR ANY PARTICULAR PURPOSE AND OF
// MERCHANTABILITY ARE HEREBY DISCLAIMED. See third_party/opennurbs/LICENSE.
use crate::Real;

pub(crate) fn saturated_hue(t: Real) -> [u8; 3] {
    let hue = (4. * std::f64::consts::PI / 3. * t) * (3. / std::f64::consts::PI);
    let rgb = match hue.floor() as u8 {
        0 => [1., hue, 0.],
        1 => [2. - hue, 1., 0.],
        2 => [0., 1., hue - 2.],
        3 => [0., 4. - hue, 1.],
        _ => [0., 0., 1.],
    };
    rgb.map(|v: Real| (255. * v).round() as u8)
}
