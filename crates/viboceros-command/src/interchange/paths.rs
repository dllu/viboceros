//! Filename arguments are not ordinary whitespace-separated modelling tokens.
use crate::CommandError;

pub(super) fn parse(input: &str, stl_format: bool) -> Result<Vec<&str>, CommandError> {
    let mut path = input.trim();
    let mut arguments = Vec::with_capacity(2);
    if stl_format {
        arguments.push("Binary");
        let end = path.find(char::is_whitespace).unwrap_or(path.len());
        let first = &path[..end];
        if first.eq_ignore_ascii_case("ascii") || first.eq_ignore_ascii_case("binary") {
            arguments[0] = first;
            path = path[end..].trim();
        }
    }
    if let Some(quoted) = path.strip_prefix('"') {
        let end = quoted
            .find('"')
            .ok_or(CommandError::Usage("unterminated quoted filename"))?;
        if end + 1 != quoted.len() {
            return Err(CommandError::Usage("unexpected text after quoted filename"));
        }
        path = &quoted[..end];
        if path.is_empty() {
            return Err(CommandError::Usage("filename must not be empty"));
        }
    }
    if !path.is_empty() {
        arguments.push(path);
    }
    Ok(arguments)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preserves_filename_content_and_only_parses_the_optional_stl_prefix() {
        assert_eq!(
            parse("  folder/two  spaces.stl  ", false).unwrap(),
            ["folder/two  spaces.stl"]
        );
        assert_eq!(
            parse(" Binary  \"C:\\parts\\two  spaces.stl\" ", true).unwrap(),
            ["Binary", "C:\\parts\\two  spaces.stl"]
        );
        assert_eq!(
            parse("\" leading and trailing spaces \"", false).unwrap(),
            [" leading and trailing spaces "]
        );
        assert_eq!(
            parse("\"Ascii file.stl\"", true).unwrap(),
            ["Binary", "Ascii file.stl"]
        );
        for input in ["\"unfinished", "\"file\" trailing", "\"\""] {
            assert!(parse(input, false).is_err());
        }
    }
}
