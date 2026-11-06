//! Command input is a token stream; newlines do not finish modeling commands.
use std::{collections::VecDeque, io::Read, path::Path};
use thiserror::Error;

pub const MAX_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_TOKENS: usize = 1_000_000;
pub const MAX_DEPTH: usize = 16;

#[derive(Debug, Error)]
pub enum MacroError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("command file must contain UTF-8 or BOM-marked UTF-16 text")]
    Encoding,
    #[error("unterminated quoted command input on line {0}")]
    Quote(usize),
    #[error("command macro exceeds its text, input, or nesting limit")]
    Limit,
    #[error("enter a filename, with balanced quotes if it contains spaces")]
    Filename,
}

pub fn path_argument(input: &str) -> Result<std::path::PathBuf, MacroError> {
    let text = input.trim();
    let text = if let Some(rest) = text.strip_prefix('"') {
        let text = rest.strip_suffix('"').ok_or(MacroError::Filename)?;
        if text.contains('"') {
            return Err(MacroError::Filename);
        }
        text
    } else {
        text
    };
    if text.is_empty() {
        return Err(MacroError::Filename);
    }
    Ok(text.into())
}

pub(super) struct ReadCommandFileCommand;
impl crate::Command for ReadCommandFileCommand {
    fn name(&self) -> &'static str {
        "ReadCommandFile"
    }
    fn records_history(&self) -> bool {
        false
    }
    fn run(
        &self,
        _doc: &mut viboceros_document::Document,
        _args: &[&str],
    ) -> Result<String, crate::CommandError> {
        Err(crate::CommandError::Usage(
            "ReadCommandFile runs through the application's command-input stream",
        ))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Token {
    /// Preserve quotation marks for the receiving prompt, including filenames.
    Input(String),
    Enter,
    EnterEnd,
    Cancel {
        leading: bool,
    },
    Pause,
    MultiPause,
    Echo(bool),
}

#[derive(Clone, Debug)]
pub struct Program {
    pub tokens: VecDeque<Token>,
}
impl Program {
    pub fn read(path: &Path) -> Result<Self, MacroError> {
        let mut bytes = Vec::new();
        std::fs::File::open(path)?
            .take((MAX_BYTES + 1) as u64)
            .read_to_end(&mut bytes)?;
        if bytes.len() > MAX_BYTES {
            return Err(MacroError::Limit);
        }
        let text = if bytes.starts_with(&[0xff, 0xfe]) || bytes.starts_with(&[0xfe, 0xff]) {
            if bytes.len() % 2 != 0 {
                return Err(MacroError::Encoding);
            }
            let little = bytes[0] == 0xff;
            let words = bytes[2..]
                .chunks_exact(2)
                .map(|b| {
                    if little {
                        u16::from_le_bytes([b[0], b[1]])
                    } else {
                        u16::from_be_bytes([b[0], b[1]])
                    }
                })
                .collect::<Vec<_>>();
            String::from_utf16(&words).map_err(|_| MacroError::Encoding)?
        } else {
            let bytes = bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(&bytes);
            std::str::from_utf8(bytes)
                .map_err(|_| MacroError::Encoding)?
                .to_owned()
        };
        Self::compile(&text)
    }
    pub fn compile(source: &str) -> Result<Self, MacroError> {
        if source.len() > MAX_BYTES {
            return Err(MacroError::Limit);
        }
        let mut tokens = VecDeque::new();
        let (mut start, mut quoted, mut comment, mut line, mut quote_line, mut seen) =
            (None, false, false, 1, 1, false);
        let emit = |word: &str, tokens: &mut VecDeque<Token>, seen: &mut bool| {
            let name = word.trim_start_matches('_');
            let token = if name.eq_ignore_ascii_case("Enter") {
                Token::Enter
            } else if name.eq_ignore_ascii_case("EnterEnd") {
                Token::EnterEnd
            } else if name.eq_ignore_ascii_case("Pause") {
                Token::Pause
            } else if name.eq_ignore_ascii_case("MultiPause") {
                Token::MultiPause
            } else if name.eq_ignore_ascii_case("NoEcho") {
                Token::Echo(false)
            } else if name.eq_ignore_ascii_case("Echo") {
                Token::Echo(true)
            } else {
                Token::Input(word.into())
            };
            if !matches!(token, Token::Echo(_)) {
                *seen = true;
            }
            tokens.push_back(token);
        };
        for (index, ch) in source.char_indices() {
            if comment {
                if ch == '\n' || ch == '\r' {
                    comment = false;
                } else {
                    continue;
                }
            }
            if !quoted && (ch.is_whitespace() || ch == ';' || ch == '!') {
                if let Some(begin) = start.take() {
                    emit(&source[begin..index], &mut tokens, &mut seen);
                }
                if ch == ';' {
                    comment = true;
                }
                if ch == '!' {
                    tokens.push_back(Token::Cancel { leading: !seen });
                    seen = true;
                }
            } else {
                start.get_or_insert(index);
                if ch == '"' {
                    quoted = !quoted;
                    if quoted {
                        quote_line = line;
                    }
                }
            }
            if ch == '\n' {
                line += 1;
            }
            if tokens.len() > MAX_TOKENS {
                return Err(MacroError::Limit);
            }
        }
        if quoted {
            return Err(MacroError::Quote(quote_line));
        }
        if let Some(begin) = start {
            emit(&source[begin..], &mut tokens, &mut seen);
        }
        if tokens.len() > MAX_TOKENS {
            return Err(MacroError::Limit);
        }
        Ok(Self { tokens })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Waiting {
    Input,
    Command,
}
#[derive(Debug)]
pub enum Action {
    Input(String),
    Cancel { abort: bool },
    Echo(bool),
}

#[derive(Default)]
pub struct Stream {
    frames: Vec<Program>,
    waiting: Option<Waiting>,
    steps: usize,
}
impl Stream {
    pub fn push(&mut self, program: Program) -> Result<(), MacroError> {
        if self.frames.len() >= MAX_DEPTH {
            return Err(MacroError::Limit);
        }
        self.frames.push(program);
        Ok(())
    }
    pub fn waiting(&self) -> Option<Waiting> {
        self.waiting
    }
    pub fn accepted_input(&mut self) {
        if self.waiting == Some(Waiting::Input) {
            self.waiting = None;
        }
    }
    pub fn finished(&self, command_active: bool) -> bool {
        self.frames.is_empty() && self.waiting.is_none() && !command_active
    }
    pub fn next(&mut self, command_active: bool) -> Result<Option<Action>, MacroError> {
        match self.waiting {
            Some(Waiting::Input) => return Ok(None),
            Some(Waiting::Command) if command_active => return Ok(None),
            _ => self.waiting = None,
        }
        loop {
            let Some(frame) = self.frames.last_mut() else {
                return Ok(None);
            };
            let Some(token) = frame.tokens.pop_front() else {
                self.frames.pop();
                continue;
            };
            self.steps += 1;
            if self.steps > MAX_TOKENS {
                return Err(MacroError::Limit);
            }
            let action = match token {
                Token::Input(text) => Action::Input(text),
                Token::Cancel { leading } => {
                    if !leading {
                        self.frames.clear();
                        self.waiting = None;
                    }
                    Action::Cancel { abort: !leading }
                }
                Token::Enter if command_active => Action::Input(String::new()),
                Token::Enter => continue,
                Token::EnterEnd if command_active => {
                    frame.tokens.push_front(Token::EnterEnd);
                    Action::Input(String::new())
                }
                Token::EnterEnd => continue,
                Token::Pause => {
                    self.waiting = Some(Waiting::Input);
                    return Ok(None);
                }
                Token::MultiPause => {
                    self.waiting = Some(Waiting::Command);
                    return Ok(None);
                }
                Token::Echo(on) => Action::Echo(on),
            };
            return Ok(Some(action));
        }
    }
}

#[cfg(test)]
mod tests;
