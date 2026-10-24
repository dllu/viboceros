//! document registry command regressions.
use super::*;

#[test]
fn dispatches_case_insensitively_and_accepts_rhino_prefix() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry.execute(&mut document, "_pT 1,2,3").unwrap();
    assert_eq!(document.objects().len(), 1);
}

#[test]
fn creates_lines_from_comma_or_space_points() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    registry.execute(&mut document, "Line 0,0 3,4,0").unwrap();
    registry.execute(&mut document, "Line 0 0 0 0 0 2").unwrap();
    assert_eq!(document.objects().len(), 2);
}

#[test]
fn failed_command_does_not_mutate_document() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    assert!(registry.execute(&mut document, "Line 0,0,0 0,0,0").is_err());
    assert_eq!(document.objects().len(), 0);
}

#[test]
fn reports_sorted_help() {
    let registry = CommandRegistry::with_builtins();
    let mut document = Document::default();
    let help = registry.execute(&mut document, "Help").unwrap();
    let names = help
        .strip_prefix("Commands: ")
        .expect("command help prefix")
        .split(", ")
        .collect::<Vec<_>>();
    let expected = include_str!("builtin_commands.txt")
        .lines()
        .collect::<Vec<_>>();
    assert_eq!(names, expected);
}
