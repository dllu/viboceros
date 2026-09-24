//! Bounded scalar arithmetic for typed coordinates. Rhino treats `1-3/4` as
//! a mixed fraction, so that spelling is one number rather than subtraction.

use viboceros_geometry::Real;

pub(super) fn evaluate(text: &str) -> Option<Real> {
    let mut parser = Parser {
        text: text.as_bytes(),
        offset: 0,
    };
    let value = parser.expression(0)?;
    (parser.offset == parser.text.len() && value.is_finite()).then_some(value)
}

struct Parser<'a> {
    text: &'a [u8],
    offset: usize,
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
        match self.peek()? {
            b'+' => {
                self.offset += 1;
                self.factor(depth + 1)
            }
            b'-' => {
                self.offset += 1;
                Some(-self.factor(depth + 1)?)
            }
            b'(' if depth < 32 => {
                self.offset += 1;
                let value = self.expression(depth + 1)?;
                (self.take() == Some(b')')).then_some(value)
            }
            b'0'..=b'9' | b'.' => self.number(),
            _ => None,
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

#[cfg(test)]
mod tests {
    use super::evaluate;

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
}
