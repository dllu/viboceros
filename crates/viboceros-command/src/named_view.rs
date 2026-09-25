//! Named-view command syntax and ordered, case-insensitive view names.
//! The saved camera type belongs to the host (GUI or headless viewport).

pub const USAGE: &str = "NamedView [List | Save name | Update name | Restore name | Delete name | Rename old | new | Duplicate source | new | MoveUp name | MoveDown name]";

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NamedViewAction {
    List,
    Save(String),
    Update(String),
    Restore(String),
    Delete(String),
    Rename { old: String, new: String },
    Duplicate { source: String, new: String },
    MoveUp(String),
    MoveDown(String),
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum NamedViewError {
    #[error("Usage: {USAGE}")]
    Usage,
    #[error("named view '{0}' does not exist")]
    Missing(String),
    #[error("named view '{0}' already exists")]
    Duplicate(String),
}

fn keyword(input: &str, expected: &str) -> bool {
    input.trim_start_matches('_').eq_ignore_ascii_case(expected)
}

fn valid_name(name: &str) -> bool {
    !name.trim().is_empty()
        && name == name.trim()
        && !name.contains('|')
        && !name.chars().any(char::is_control)
}

fn name(input: &str) -> Result<String, NamedViewError> {
    let name = input.trim();
    if !valid_name(name) {
        return Err(NamedViewError::Usage);
    }
    Ok(name.to_owned())
}

pub fn parse(input: &str) -> Option<Result<NamedViewAction, NamedViewError>> {
    let input = input.trim();
    let (command, rest) = input.split_once(char::is_whitespace).unwrap_or((input, ""));
    if !command
        .trim_start_matches(['\'', '_', '-'])
        .eq_ignore_ascii_case("NamedView")
    {
        return None;
    }
    let rest = rest.trim();
    if rest.is_empty() || keyword(rest, "List") {
        return Some(Ok(NamedViewAction::List));
    }
    let (verb, value) = rest.split_once(char::is_whitespace).unwrap_or((rest, ""));
    let value = value.trim();
    Some(if keyword(verb, "Save") {
        name(value).map(NamedViewAction::Save)
    } else if keyword(verb, "Update") {
        name(value).map(NamedViewAction::Update)
    } else if keyword(verb, "Restore") {
        name(value).map(NamedViewAction::Restore)
    } else if keyword(verb, "Delete") {
        name(value).map(NamedViewAction::Delete)
    } else if keyword(verb, "MoveUp") {
        name(value).map(NamedViewAction::MoveUp)
    } else if keyword(verb, "MoveDown") {
        name(value).map(NamedViewAction::MoveDown)
    } else if keyword(verb, "Rename") || keyword(verb, "Duplicate") {
        value
            .split_once('|')
            .ok_or(NamedViewError::Usage)
            .and_then(|(left, right)| {
                let source = name(left)?;
                let new = name(right)?;
                Ok(if keyword(verb, "Rename") {
                    NamedViewAction::Rename { old: source, new }
                } else {
                    NamedViewAction::Duplicate { source, new }
                })
            })
    } else {
        Err(NamedViewError::Usage)
    })
}

#[derive(Clone, Debug)]
struct Entry<S> {
    name: String,
    snapshot: S,
}

#[derive(Clone, Debug)]
pub struct NamedViews<S> {
    entries: Vec<Entry<S>>,
}

impl<S> Default for NamedViews<S> {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
        }
    }
}

impl<S> NamedViews<S> {
    fn index(&self, name: &str) -> Option<usize> {
        let folded = name.to_lowercase();
        self.entries
            .iter()
            .position(|entry| entry.name.to_lowercase() == folded)
    }

    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.entries.iter().map(|entry| entry.name.as_str())
    }

    pub fn entries(&self) -> impl Iterator<Item = (&str, &S)> {
        self.entries
            .iter()
            .map(|entry| (entry.name.as_str(), &entry.snapshot))
    }

    pub fn get(&self, name: &str) -> Result<&S, NamedViewError> {
        self.index(name)
            .map(|i| &self.entries[i].snapshot)
            .ok_or_else(|| NamedViewError::Missing(name.to_owned()))
    }

    pub fn save(&mut self, name: String, snapshot: S) -> Result<(), NamedViewError> {
        if !valid_name(&name) {
            return Err(NamedViewError::Usage);
        }
        if self.index(&name).is_some() {
            return Err(NamedViewError::Duplicate(name));
        }
        self.entries.push(Entry { name, snapshot });
        Ok(())
    }

    pub fn update(&mut self, name: &str, snapshot: S) -> Result<(), NamedViewError> {
        let index = self
            .index(name)
            .ok_or_else(|| NamedViewError::Missing(name.to_owned()))?;
        self.entries[index].snapshot = snapshot;
        Ok(())
    }

    pub fn delete(&mut self, name: &str) -> Result<(), NamedViewError> {
        let index = self
            .index(name)
            .ok_or_else(|| NamedViewError::Missing(name.to_owned()))?;
        self.entries.remove(index);
        Ok(())
    }

    pub fn rename(&mut self, old: &str, new: String) -> Result<(), NamedViewError> {
        if !valid_name(&new) {
            return Err(NamedViewError::Usage);
        }
        let index = self
            .index(old)
            .ok_or_else(|| NamedViewError::Missing(old.to_owned()))?;
        if self.index(&new).is_some_and(|existing| existing != index) {
            return Err(NamedViewError::Duplicate(new));
        }
        self.entries[index].name = new;
        Ok(())
    }

    pub fn move_by(&mut self, name: &str, delta: isize) -> Result<(), NamedViewError> {
        let index = self
            .index(name)
            .ok_or_else(|| NamedViewError::Missing(name.to_owned()))?;
        let destination = index
            .saturating_add_signed(delta)
            .min(self.entries.len() - 1);
        self.entries.swap(index, destination);
        Ok(())
    }
}

impl<S: Clone> NamedViews<S> {
    pub fn duplicate(&mut self, source: &str, new: String) -> Result<(), NamedViewError> {
        let snapshot = self.get(source)?.clone();
        self.save(new, snapshot)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_can_contain_spaces_and_edits_preserve_order() {
        assert_eq!(
            parse("NamedView Save Upper left"),
            Some(Ok(NamedViewAction::Save("Upper left".into())))
        );
        assert_eq!(
            parse("_NamedView Rename Upper left | Detail"),
            Some(Ok(NamedViewAction::Rename {
                old: "Upper left".into(),
                new: "Detail".into(),
            }))
        );
        assert_eq!(
            parse("NamedView Rename a | "),
            Some(Err(NamedViewError::Usage))
        );
        let mut views = NamedViews::default();
        views.save("Upper left".into(), 1).unwrap();
        views.save("Right".into(), 2).unwrap();
        assert_eq!(
            views.save("upper LEFT".into(), 3),
            Err(NamedViewError::Duplicate("upper LEFT".into()))
        );
        views.duplicate("Upper left", "Copy".into()).unwrap();
        views.update("copy", 4).unwrap();
        views.move_by("Copy", -1).unwrap();
        assert_eq!(
            views.names().collect::<Vec<_>>(),
            vec!["Upper left", "Copy", "Right"]
        );
        views.rename("copy", "Detail".into()).unwrap();
        assert_eq!(*views.get("detail").unwrap(), 4);
        views.delete("Upper left").unwrap();
        assert_eq!(views.names().collect::<Vec<_>>(), vec!["Detail", "Right"]);
        views.save("Überblick".into(), 5).unwrap();
        assert_eq!(*views.get("überblick").unwrap(), 5);
        assert_eq!(
            views.save("invalid | name".into(), 6),
            Err(NamedViewError::Usage)
        );
    }
}
