//! Shared case-insensitive star/question matching for object and layer names.

#[derive(Clone, Debug)]
pub(super) struct CaseInsensitiveWildcard {
    pattern: Vec<char>,
}

impl CaseInsensitiveWildcard {
    pub(super) fn new(pattern: &str) -> Self {
        Self {
            pattern: pattern.to_lowercase().chars().collect(),
        }
    }

    pub(super) fn matches(&self, candidate: &str) -> bool {
        if candidate.is_ascii() {
            // ASCII lowercase is one-to-one and context independent. Borrow
            // the bytes and fold on demand instead of allocating per object.
            let bytes = candidate.as_bytes();
            return self.matches_characters(bytes.len(), |index| {
                char::from(bytes[index].to_ascii_lowercase())
            });
        }
        // Keep whole-string Unicode conversion: scalar-wise folding would
        // change contextual mappings such as the Greek final sigma.
        let candidate = candidate.to_lowercase().chars().collect::<Vec<_>>();
        self.matches_characters(candidate.len(), |index| candidate[index])
    }

    fn matches_characters(
        &self,
        candidate_length: usize,
        character_at: impl Fn(usize) -> char,
    ) -> bool {
        let mut pattern_index = 0;
        let mut candidate_index = 0;
        let mut star_index = None;
        let mut star_candidate_index = 0;
        while candidate_index < candidate_length {
            if pattern_index < self.pattern.len()
                && self.pattern[pattern_index] != '*'
                && (self.pattern[pattern_index] == '?'
                    || self.pattern[pattern_index] == character_at(candidate_index))
            {
                pattern_index += 1;
                candidate_index += 1;
            } else if pattern_index < self.pattern.len() && self.pattern[pattern_index] == '*' {
                star_index = Some(pattern_index);
                pattern_index += 1;
                star_candidate_index = candidate_index;
            } else if let Some(star) = star_index {
                pattern_index = star + 1;
                star_candidate_index += 1;
                candidate_index = star_candidate_index;
            } else {
                return false;
            }
        }
        while pattern_index < self.pattern.len() && self.pattern[pattern_index] == '*' {
            pattern_index += 1;
        }
        pattern_index == self.pattern.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Independent dynamic-programming reference: each cell represents matching
    // two prefixes, with no greedy cursor or last-star backtracking state.
    fn reference_matches(pattern: &str, candidate: &str) -> bool {
        let pattern = pattern.to_lowercase().chars().collect::<Vec<_>>();
        let candidate = candidate.to_lowercase().chars().collect::<Vec<_>>();
        let mut prefixes = vec![vec![false; candidate.len() + 1]; pattern.len() + 1];
        prefixes[0][0] = true;
        for (i, token) in pattern.iter().enumerate() {
            prefixes[i + 1][0] = *token == '*' && prefixes[i][0];
            for (j, character) in candidate.iter().enumerate() {
                prefixes[i + 1][j + 1] = match token {
                    '*' => prefixes[i][j + 1] || prefixes[i + 1][j],
                    '?' => prefixes[i][j],
                    literal => literal == character && prefixes[i][j],
                };
            }
        }
        prefixes[pattern.len()][candidate.len()]
    }

    fn words(alphabet: &[char], max_length: usize) -> Vec<String> {
        let mut all = vec![String::new()];
        let mut previous = vec![String::new()];
        for _ in 0..max_length {
            let mut next = Vec::new();
            for prefix in &previous {
                for character in alphabet {
                    next.push(format!("{prefix}{character}"));
                }
            }
            all.extend(next.iter().cloned());
            previous = next;
        }
        all
    }

    #[test]
    fn greedy_matches_exhaustive_prefix_reference() {
        let patterns = words(&['a', 'B', '*', '?'], 4);
        let candidates = words(&['a', 'b', '*', '?'], 4);
        assert_eq!(patterns.len() * candidates.len(), 116_281);
        for pattern in patterns {
            let matcher = CaseInsensitiveWildcard::new(&pattern);
            for candidate in &candidates {
                assert_eq!(
                    matcher.matches(candidate),
                    reference_matches(&pattern, candidate),
                    "pattern {pattern:?}, candidate {candidate:?}"
                );
            }
        }
    }

    #[test]
    fn ascii_fast_path_agrees_with_reference_for_every_byte() {
        for pattern in ["*", "?", "??", "*?*", "a*B", "K*", "İ*", "Σ*"] {
            let matcher = CaseInsensitiveWildcard::new(pattern);
            for byte in 0..=127_u8 {
                for candidate in [
                    char::from(byte).to_string(),
                    format!("A{}B", char::from(byte)),
                ] {
                    assert!(candidate.is_ascii());
                    assert_eq!(
                        matcher.matches(&candidate),
                        reference_matches(pattern, &candidate),
                        "{pattern:?}, {candidate:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn unicode_and_ascii_paths_agree_with_prefix_reference() {
        let patterns = words(&['A', 'İ', 'Σ', 'K', '*', '?'], 3);
        let candidates = words(&['a', 'i', '\u{307}', 'σ', 'ς', 'k', '*', '?'], 2);
        for pattern in patterns {
            let matcher = CaseInsensitiveWildcard::new(&pattern);
            for candidate in &candidates {
                assert_eq!(
                    matcher.matches(candidate),
                    reference_matches(&pattern, candidate),
                    "{pattern:?}, {candidate:?}"
                );
            }
        }
        // Assert contextual and expanding mappings explicitly as well as
        // comparing algorithms that share Rust's lowercase conversion.
        assert!(CaseInsensitiveWildcard::new("ος").matches("ΟΣ"));
        assert!(!CaseInsensitiveWildcard::new("οσ").matches("ΟΣ"));
        assert!(!CaseInsensitiveWildcard::new("?").matches("İ"));
        assert!(CaseInsensitiveWildcard::new("??").matches("İ"));
        assert!(CaseInsensitiveWildcard::new("K").matches("K"));
    }

    #[test]
    fn long_backtracking_and_unicode_match_prefix_reference() {
        for (pattern, candidate) in [
            (
                format!("*{}b", "a".repeat(64)),
                format!("*{}c", "a".repeat(256)),
            ),
            ("*Å?*部品".to_owned(), "*å\t**部品".to_owned()),
            ("*?İ*".to_owned(), "*İ".to_owned()),
        ] {
            assert_eq!(
                CaseInsensitiveWildcard::new(&pattern).matches(&candidate),
                reference_matches(&pattern, &candidate)
            );
        }
    }
}
