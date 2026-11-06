use super::*;

#[test]
fn whitespace_comments_and_quoted_punctuation_retain_the_native_input_boundaries() {
    let p = Program::compile(
        "; header\n!_Line\n0,0,0 4,3,2 ; tail\n\n_-Layer _New \"a; ! b\" _Enter\n",
    )
    .unwrap();
    assert_eq!(
        p.tokens,
        VecDeque::from([
            Token::Cancel { leading: true },
            Token::Input("_Line".into()),
            Token::Input("0,0,0".into()),
            Token::Input("4,3,2".into()),
            Token::Input("_-Layer".into()),
            Token::Input("_New".into()),
            Token::Input("\"a; ! b\"".into()),
            Token::Enter,
        ])
    );
    let p = Program::compile("_NoEcho !_Point 1,2,3 ! _Point 4,5,6").unwrap();
    assert!(matches!(p.tokens[1], Token::Cancel { leading: true }));
    assert!(matches!(p.tokens[4], Token::Cancel { leading: false }));
    assert!(matches!(
        Program::compile("\n\"unfinished").unwrap_err(),
        MacroError::Quote(2)
    ));
}

#[test]
fn bom_encodings_and_windows_paths_keep_literal_text() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("macro.txt");
    let text = "_-ReadCommandFile \"C:\\parts\\two words.txt\"\n";
    let expected = Program::compile(text).unwrap().tokens;
    for (bom, little) in [([0xff, 0xfe], true), ([0xfe, 0xff], false)] {
        let mut bytes = bom.to_vec();
        for word in text.encode_utf16() {
            bytes.extend(if little {
                word.to_le_bytes()
            } else {
                word.to_be_bytes()
            });
        }
        std::fs::write(&path, bytes).unwrap();
        assert_eq!(Program::read(&path).unwrap().tokens, expected);
    }
    let mut bytes = vec![0xef, 0xbb, 0xbf];
    bytes.extend(text.as_bytes());
    std::fs::write(&path, bytes).unwrap();
    assert_eq!(Program::read(&path).unwrap().tokens, expected);
    std::fs::write(&path, [0xff, 0xfe, 0]).unwrap();
    assert!(matches!(Program::read(&path), Err(MacroError::Encoding)));
}

#[test]
fn pauses_wait_for_admitted_input_and_enter_never_repeats_a_command() {
    let mut s = Stream::default();
    s.push(Program::compile("_Enter _Line _Pause 4,3,2 _MultiPause _Point 9,9,9").unwrap())
        .unwrap();
    assert!(matches!(s.next(false).unwrap(),Some(Action::Input(t)) if t=="_Line"));
    assert!(s.next(true).unwrap().is_none());
    assert_eq!(s.waiting(), Some(Waiting::Input));
    assert!(s.next(true).unwrap().is_none());
    s.accepted_input();
    assert!(matches!(s.next(true).unwrap(),Some(Action::Input(t)) if t=="4,3,2"));
    assert!(s.next(true).unwrap().is_none());
    s.accepted_input();
    assert!(s.next(true).unwrap().is_none());
    assert!(matches!(s.next(false).unwrap(),Some(Action::Input(t)) if t=="_Point"));
    assert!(matches!(s.next(true).unwrap(),Some(Action::Input(t)) if t=="9,9,9"));
    assert!(s.next(false).unwrap().is_none());
    assert!(s.finished(false));
}

#[test]
fn nested_files_resume_the_parent_and_midstream_cancellation_discards_the_tail() {
    let mut s = Stream::default();
    s.push(Program::compile("_Point 9,9,9").unwrap()).unwrap();
    s.push(Program::compile("! _Line 0,0,0 4,3,2").unwrap())
        .unwrap();
    assert!(matches!(
        s.next(false).unwrap(),
        Some(Action::Cancel { abort: false })
    ));
    for _ in 0..3 {
        assert!(matches!(s.next(true).unwrap(), Some(Action::Input(_))));
    }
    assert!(matches!(s.next(false).unwrap(),Some(Action::Input(t)) if t=="_Point"));
    let mut s = Stream::default();
    s.push(Program::compile("_Point 1,2,3 ! _Point 4,5,6").unwrap())
        .unwrap();
    s.next(false).unwrap();
    s.next(true).unwrap();
    assert!(matches!(
        s.next(false).unwrap(),
        Some(Action::Cancel { abort: true })
    ));
    assert!(s.next(false).unwrap().is_none());
    assert!(s.finished(false));
}
