//! Exact positive-ray recognition for the zero-angle limit.
use super::*;
use crate::exact_scalar::rational;

pub(super) fn same(a: &[[Real; 3]], b: &[[Real; 3]]) -> bool {
    let Some(reference) = a.iter().copied().find(|v| *v != [0.; 3]) else {
        return false;
    };
    let axis = reference.iter().position(|v| *v != 0.).unwrap();
    let exact = reference.map(rational);
    for values in [a, b] {
        let mut any = false;
        for &value in values {
            if value == [0.; 3] {
                continue;
            }
            any = true;
            if value[axis] == 0.
                || value[axis].is_sign_positive() != reference[axis].is_sign_positive()
            {
                return false;
            }
            let value = value.map(rational);
            for i in 0..3 {
                if &value[i] * &exact[axis] != &exact[i] * &value[axis] {
                    return false;
                }
            }
        }
        if !any {
            return false;
        }
    }
    true
}
