//! Bounded scalar arithmetic for typed coordinates. Rhino treats `1-3/4` as
//! a mixed fraction, so that spelling is one number rather than subtraction.

use viboceros_geometry::{LengthUnitSystem, Real};

#[cfg(test)]
pub(super) fn evaluate(text: &str) -> Option<Real> {
    evaluate_in_units(text, &LengthUnitSystem::Millimeters)
}

pub(super) fn evaluate_in_units(text: &str, units: &LengthUnitSystem) -> Option<Real> {
    let mut parser = Parser {
        text: text.as_bytes(),
        offset: 0,
        units,
    };
    let value = parser.expression(0)?;
    (parser.offset == parser.text.len() && value.is_finite()).then_some(value)
}

struct Parser<'a> {
    text: &'a [u8],
    offset: usize,
    units: &'a LengthUnitSystem,
}

impl Parser<'_> {
    fn expression(&mut self, depth: usize) -> Option<Real> {
        let mut value = self.term(depth)?;
        loop {
            let next = match self.peek() {
                Some(b'+') => 1.0,
                Some(b'-') => -1.0,
                _ => break,
            };
            self.offset += 1;
            value += next * self.term(depth)?;
            if !value.is_finite() {
                return None;
            }
        }
        Some(value)
    }

    fn term(&mut self, depth: usize) -> Option<Real> {
        let mut value = self.factor(depth)?;
        loop {
            let divide = match self.peek() {
                Some(b'*') => false,
                Some(b'/') => true,
                _ => break,
            };
            self.offset += 1;
            let rhs = self.factor(depth)?;
            value = if divide { value / rhs } else { value * rhs };
            if !value.is_finite() {
                return None;
            }
        }
        Some(value)
    }

    fn factor(&mut self, depth: usize) -> Option<Real> {
        if depth > 32 {
            return None;
        }
        let value = match self.peek()? {
            b'+' => {
                self.offset += 1;
                return self.factor(depth + 1);
            }
            b'-' => {
                self.offset += 1;
                return Some(-self.factor(depth + 1)?);
            }
            b'(' if depth < 32 => {
                self.offset += 1;
                let value = self.expression(depth + 1)?;
                if self.take() != Some(b')') {
                    return None;
                }
                value
            }
            b'0'..=b'9' | b'.' => self.number()?,
            b'a'..=b'z' | b'A'..=b'Z' => self.named(depth)?,
            _ => return None,
        };
        if self.peek() == Some(b'\'') {
            self.offset += 1;
            let feet = value * LengthUnitSystem::Feet.scale_to(self.units).ok()?;
            if self.peek().is_some_and(|c| c.is_ascii_digit() || c == b'.') {
                let inches = self.number()? * LengthUnitSystem::Inches.scale_to(self.units).ok()?;
                if self.take() != Some(b'"') {
                    return None;
                }
                return Some(feet + inches);
            }
            return Some(feet);
        }
        if self.peek() == Some(b'"') {
            self.offset += 1;
            return Some(value * LengthUnitSystem::Inches.scale_to(self.units).ok()?);
        }
        if self.peek().is_some_and(|c| c.is_ascii_alphabetic()) {
            let suffix = self.identifier()?.to_ascii_lowercase();
            let factor = match suffix.as_str() {
                "d" | "degrees" => Some(std::f64::consts::PI / 180.0),
                "radians" => Some(1.0),
                "gradians" => Some(std::f64::consts::PI / 200.0),
                _ => {
                    // Rhino's point prompt accepts a length suffix at the end
                    // of a value, but rejects `1m+20` and `1m+20cm`.
                    if matches!(self.peek(), Some(b'+' | b'-' | b'*' | b'/')) {
                        return None;
                    }
                    length_unit(&suffix).and_then(|source| source.scale_to(self.units).ok())
                }
            };
            Some(value * factor?)
        } else {
            Some(value)
        }
    }

    fn named(&mut self, depth: usize) -> Option<Real> {
        let name = self.identifier()?.to_ascii_lowercase();
        if name == "pi" {
            return Some(std::f64::consts::PI);
        }
        if self.take() != Some(b'(') {
            return None;
        }
        let first = self.expression(depth + 1)?;
        let value = match name.as_str() {
            "atan2" | "pow" => {
                if self.take() != Some(b',') {
                    return None;
                }
                let second = self.expression(depth + 1)?;
                if name == "atan2" {
                    first.atan2(second)
                } else {
                    first.powf(second)
                }
            }
            "sin" => first.sin(),
            "cos" => first.cos(),
            "tan" => first.tan(),
            "asin" => first.asin(),
            "acos" => first.acos(),
            "atan" => first.atan(),
            "ln" => first.ln(),
            "log10" => first.log10(),
            "exp" => first.exp(),
            "sinh" => first.sinh(),
            "cosh" => first.cosh(),
            "tanh" => first.tanh(),
            "sqrt" => first.sqrt(),
            _ => return None,
        };
        (self.take() == Some(b')') && value.is_finite()).then_some(value)
    }

    fn identifier(&mut self) -> Option<&str> {
        let start = self.offset;
        while self.peek().is_some_and(|c| c.is_ascii_alphanumeric()) {
            self.offset += 1;
        }
        if self.offset == start {
            None
        } else {
            std::str::from_utf8(&self.text[start..self.offset]).ok()
        }
    }

    fn number(&mut self) -> Option<Real> {
        let start = self.offset;
        while self.peek().is_some_and(|c| c.is_ascii_digit()) {
            self.offset += 1;
        }
        let integer_end = self.offset;
        if self.peek() == Some(b'.') {
            self.offset += 1;
            while self.peek().is_some_and(|c| c.is_ascii_digit()) {
                self.offset += 1;
            }
        }
        if self.offset == start || (self.offset == start + 1 && self.text[start] == b'.') {
            return None;
        }
        if matches!(self.peek(), Some(b'e' | b'E')) {
            self.offset += 1;
            if matches!(self.peek(), Some(b'+' | b'-')) {
                self.offset += 1;
            }
            let exponent_start = self.offset;
            while self.peek().is_some_and(|c| c.is_ascii_digit()) {
                self.offset += 1;
            }
            if self.offset == exponent_start {
                return None;
            }
        }
        let whole = std::str::from_utf8(&self.text[start..self.offset])
            .ok()?
            .parse::<Real>()
            .ok()?;
        if integer_end == self.offset && self.peek() == Some(b'-') {
            let separator = self.offset;
            self.offset += 1;
            let numerator_start = self.offset;
            while self.peek().is_some_and(|c| c.is_ascii_digit()) {
                self.offset += 1;
            }
            let numerator_end = self.offset;
            if numerator_end > numerator_start && self.peek() == Some(b'/') {
                self.offset += 1;
                let denominator_start = self.offset;
                while self.peek().is_some_and(|c| c.is_ascii_digit()) {
                    self.offset += 1;
                }
                if self.offset > denominator_start {
                    let numerator = std::str::from_utf8(&self.text[numerator_start..numerator_end])
                        .ok()?
                        .parse::<Real>()
                        .ok()?;
                    let denominator =
                        std::str::from_utf8(&self.text[denominator_start..self.offset])
                            .ok()?
                            .parse::<Real>()
                            .ok()?;
                    return (denominator != 0.0).then_some(whole + numerator / denominator);
                }
            }
            self.offset = separator;
        }
        if integer_end == self.offset && self.peek() == Some(b'/') {
            let separator = self.offset;
            self.offset += 1;
            let denominator_start = self.offset;
            while self.peek().is_some_and(|c| c.is_ascii_digit()) {
                self.offset += 1;
            }
            if self.offset > denominator_start && !matches!(self.peek(), Some(b'.' | b'e' | b'E')) {
                let denominator = std::str::from_utf8(&self.text[denominator_start..self.offset])
                    .ok()?
                    .parse::<Real>()
                    .ok()?;
                return (denominator != 0.0).then_some(whole / denominator);
            }
            self.offset = separator;
        }
        Some(whole)
    }

    fn peek(&self) -> Option<u8> {
        self.text.get(self.offset).copied()
    }

    fn take(&mut self) -> Option<u8> {
        let byte = self.peek()?;
        self.offset += 1;
        Some(byte)
    }
}

fn length_unit(name: &str) -> Option<LengthUnitSystem> {
    Some(match name {
        "mm" | "millimeter" | "millimeters" | "millimetre" | "millimetres" => {
            LengthUnitSystem::Millimeters
        }
        "cm" | "centimeter" | "centimeters" | "centimetre" | "centimetres" => {
            LengthUnitSystem::Centimeters
        }
        "m" | "meter" | "meters" | "metre" | "metres" => LengthUnitSystem::Meters,
        "in" | "inch" | "inches" => LengthUnitSystem::Inches,
        "ft" | "foot" | "feet" => LengthUnitSystem::Feet,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::{evaluate, evaluate_in_units};
    use viboceros_geometry::LengthUnitSystem;

    #[test]
    fn fraction_precedence_and_errors() {
        for (input, expected) in [
            ("5/16", 0.3125),
            ("1-3/4", 1.75),
            ("-1-3/4", -1.75),
            ("1+2", 3.0),
            ("4-1", 3.0),
            ("2*(3+4)", 14.0),
            ("(10-3)/7", 1.0),
            ("1+1/2", 1.5),
            ("1e-3+2e-3", 0.003),
        ] {
            assert_eq!(evaluate(input), Some(expected), "{input}");
        }
        for input in [
            "", ".", "1/0", "1-2/0", "1e", "2*(3+4", "1+", "nan", "1e309",
        ] {
            assert_eq!(evaluate(input), None, "{input}");
        }
    }

    #[test]
    fn constants_functions_and_angle_units() {
        for (input, expected) in [
            ("pi", std::f64::consts::PI),
            ("10*sin(30degrees)", 5.0),
            ("10*cos(30degrees)", 5.0 * 3.0_f64.sqrt()),
            ("atan2(1,1)", std::f64::consts::FRAC_PI_4),
            ("pow(2,3)", 8.0),
            ("sqrt(9)", 3.0),
            ("ln(exp(1))", 1.0),
            ("log10(100)", 2.0),
            ("asin(1)", std::f64::consts::FRAC_PI_2),
            ("acos(0)", std::f64::consts::FRAC_PI_2),
            ("atan(1)", std::f64::consts::FRAC_PI_4),
            ("sinh(0)", 0.0),
            ("cosh(0)", 1.0),
            ("tanh(0)", 0.0),
            ("100gradians", std::f64::consts::FRAC_PI_2),
            ("pi/2radians", std::f64::consts::FRAC_PI_2),
        ] {
            let actual = evaluate(input).unwrap();
            assert!((actual - expected).abs() < 2e-14, "{input}: {actual}");
        }
        for input in [
            "unknown(1)",
            "sin()",
            "pow(2)",
            "atan2(1,)",
            "sqrt(-1)",
            "ln(0)",
            "acos(2)",
        ] {
            assert_eq!(evaluate(input), None, "{input}");
        }
    }

    #[test]
    fn length_suffixes_follow_model_units() {
        for (units, expected) in [
            (LengthUnitSystem::Millimeters, [270.0, 1000.0, 50.8, 914.4]),
            (LengthUnitSystem::Meters, [0.27, 1.0, 0.0508, 0.9144]),
            (
                LengthUnitSystem::Inches,
                [270.0 / 25.4, 1000.0 / 25.4, 2.0, 36.0],
            ),
        ] {
            for (input, target) in ["27cm", "1m", "2in", "3ft"].into_iter().zip(expected) {
                let actual = evaluate_in_units(input, &units).unwrap();
                assert!(
                    (actual - target).abs() < 1e-12,
                    "{input} in {units:?}: {actual}"
                );
            }
        }
        let mm = LengthUnitSystem::Millimeters;
        for (input, target) in [
            ("1'2-3/4\"", 374.65),
            ("1/2in", 12.7),
            ("+16'5\"", 5003.8),
            ("(1+0.2)m", 1200.0),
        ] {
            assert!(
                (evaluate_in_units(input, &mm).unwrap() - target).abs() < 1e-12,
                "{input}"
            );
        }
        for input in ["1m+20", "1m+20cm", "1m*2"] {
            assert_eq!(evaluate_in_units(input, &mm), None, "{input}");
        }
    }
}
