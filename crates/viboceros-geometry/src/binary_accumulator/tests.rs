use super::*;

// Deliberately use individual binary digits, not machine-word carry arithmetic,
// so the reference does not repeat add_product/add_word's implementation.
fn reference_add(words: &[u64; 66], product: u128, shift: usize) -> [u64; 66] {
    let mut bits = words
        .iter()
        .flat_map(|word| (0..64).map(move |bit| word & (1_u64 << bit) != 0))
        .collect::<Vec<_>>();
    for bit in 0..128 {
        if product & (1_u128 << bit) == 0 {
            continue;
        }
        let mut position = shift + bit;
        while bits[position] {
            bits[position] = false;
            position += 1;
        }
        bits[position] = true;
    }
    std::array::from_fn(|word| {
        (0..64).fold(0, |value, bit| {
            value | (u64::from(bits[word * 64 + bit]) << bit)
        })
    })
}

#[test]
fn shifted_products_match_an_independent_bitwise_adder() {
    let mut state = 0x9182_7346_abcd_ef01_u64;
    let mut random: [u64; 66] = std::array::from_fn(|_| {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
        state
    });
    let mut carry_chain = [u64::MAX; 66];
    // Reserve a complete high word. Even the largest tested shifted product
    // and a carry through every lower word then fit the reference array.
    random[65] = 0;
    carry_chain[65] = 0;
    for original in [random, carry_chain] {
        for base in [0, 1, 31, 63] {
            for offset in 0..64 {
                let shift = base * 64 + offset;
                for product in [0, 1, u64::MAX as u128, 1_u128 << 64, u128::MAX] {
                    let expected = reference_add(&original, product, shift);
                    let mut actual = original;
                    add_product(&mut actual, product, shift);
                    assert_eq!(actual, expected, "shift={shift}, product={product:x}");
                }
            }
        }
    }
}

#[test]
fn signed_subtraction_propagates_borrows_across_all_sum_limbs() {
    for highest in [64, 128, 1024, 1074, 2048, 2098, 2161] {
        let mut power = [0_u64; 34];
        power[highest / 64] = 1_u64 << (highest % 64);
        let mut predecessor = [0_u64; 34];
        predecessor[..highest / 64].fill(u64::MAX);
        predecessor[highest / 64] = (1_u64 << (highest % 64)) - 1;
        // 2^highest - (2^highest - 1) is one accumulator quantum, even when
        // neither magnitude is individually representable as a finite f64.
        assert_eq!(finish::<34, 1074>(power, predecessor), f64::from_bits(1));
        assert_eq!(finish::<34, 1074>(predecessor, power), -f64::from_bits(1));
        assert_eq!(finish::<34, 1074>(power, power).to_bits(), 0);
    }
}

fn check_rounding_offsets<const QUANTUM: usize>() {
    for offset in 0..64 {
        let shift = QUANTUM - 1074 + 65 + offset;
        let highest = shift + 52;
        let base = 2_f64.powi(highest as i32 - QUANTUM as i32);
        let mut magnitude = [0_u64; 66];
        magnitude[highest / 64] = 1_u64 << (highest % 64);
        magnitude[(shift - 1) / 64] |= 1_u64 << ((shift - 1) % 64);
        // Even significand + exactly half an ulp rounds down.
        assert_eq!(finish::<66, QUANTUM>(magnitude, [0; 66]), base);
        // A remote low bit changes a tie into a strict round-up.
        magnitude[0] |= 1;
        assert_eq!(
            finish::<66, QUANTUM>(magnitude, [0; 66]),
            f64::from_bits(base.to_bits() + 1)
        );
        // At an odd significand the exact tie instead rounds up to even.
        magnitude[0] &= !1;
        magnitude[shift / 64] |= 1_u64 << (shift % 64);
        assert_eq!(
            finish::<66, QUANTUM>(magnitude, [0; 66]),
            f64::from_bits(base.to_bits() + 2)
        );
    }
}

#[test]
fn guard_and_sticky_bits_round_correctly_at_every_word_offset() {
    check_rounding_offsets::<1074>();
    check_rounding_offsets::<2148>();
}
