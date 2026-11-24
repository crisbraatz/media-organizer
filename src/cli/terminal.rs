use crossterm::cursor::MoveToColumn;
use crossterm::event::{
    self, DisableBracketedPaste, EnableBracketedPaste, Event, KeyCode, KeyEvent, KeyEventKind,
    KeyModifiers,
};
use crossterm::terminal::{self, Clear, ClearType};
use crossterm::{execute, queue};
use std::io::{self, StdoutLock, Write};
use std::path::{MAIN_SEPARATOR, Path, PathBuf, is_separator};

const MAX_PATH_INPUT: usize = 32_768;

struct RawPrompt {
    raw: bool,
    paste: bool,
}

impl RawPrompt {
    fn begin(output: &mut StdoutLock<'_>) -> io::Result<Self> {
        terminal::enable_raw_mode()?;
        let mut guard = Self {
            raw: true,
            paste: true,
        };
        if let Err(error) = execute!(output, EnableBracketedPaste) {
            if cfg!(windows) && error.kind() == io::ErrorKind::Unsupported {
                guard.paste = false;
            } else {
                // Use the existing stdout lock while cleaning up a failed setup.
                let _ = guard.restore(output);
                return Err(error);
            }
        }
        Ok(guard)
    }

    fn restore(&mut self, output: &mut StdoutLock<'_>) -> io::Result<()> {
        let paste = if self.paste {
            execute!(output, DisableBracketedPaste).inspect(|()| self.paste = false)
        } else {
            Ok(())
        };
        let raw = if self.raw {
            terminal::disable_raw_mode().inspect(|()| self.raw = false)
        } else {
            Ok(())
        };
        paste.and(raw)
    }
}

impl Drop for RawPrompt {
    fn drop(&mut self) {
        if self.raw || self.paste {
            let _ = self.restore(&mut io::stdout().lock());
        }
    }
}

fn prompt<T>(run: impl FnOnce(&mut StdoutLock<'_>) -> io::Result<T>) -> io::Result<T> {
    let mut output = io::stdout().lock();
    let mut guard = RawPrompt::begin(&mut output)?;
    let result = run(&mut output);
    let restore = guard.restore(&mut output);
    drop(output);
    restore?;
    result
}

fn accepts(key: KeyEvent) -> bool {
    key.kind == KeyEventKind::Press
        || (key.kind == KeyEventKind::Repeat
            && !matches!(key.code, KeyCode::Enter | KeyCode::Esc)
            && !key.modifiers.contains(KeyModifiers::CONTROL))
}

fn cancelled(key: KeyEvent) -> bool {
    key.code == KeyCode::Esc
        || (key.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(key.code, KeyCode::Char('c' | 'C' | 'd' | 'D')))
}

pub(super) fn escaped(input: &str) -> String {
    let mut escaped = String::new();
    for ch in input.chars() {
        if ch.is_control()
            || matches!(ch, '\u{061c}' | '\u{200e}'..='\u{200f}' | '\u{2028}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
        {
            escaped.extend(ch.escape_default());
        } else {
            escaped.push(ch);
        }
    }
    escaped
}

fn complete(input: &str) -> Option<String> {
    let prefix_end = input.rfind(is_separator).map_or(0, |index| index + 1);
    let (prefix, partial) = input.split_at(prefix_end);
    let parent = if prefix.is_empty() { "." } else { prefix };
    let mut candidates: Vec<_> = std::fs::read_dir(parent)
        .ok()?
        .filter_map(Result::ok)
        .filter(|entry| entry.path().is_dir())
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|name| name.starts_with(partial))
        .collect();
    candidates.sort_unstable();
    match candidates.as_slice() {
        [] => None,
        [only] => Some(format!("{prefix}{only}{MAIN_SEPARATOR}")),
        [first, .., last] => {
            let shared: String = first
                .chars()
                .zip(last.chars())
                .take_while(|(a, b)| a == b)
                .map(|(ch, _)| ch)
                .collect();
            Some(format!("{prefix}{shared}"))
        }
    }
}

fn terminal_width() -> usize {
    match terminal::size() {
        Ok((cols, _)) if cols > 0 => usize::from(cols),
        _ => 80,
    }
}

fn redraw(output: &mut StdoutLock<'_>, label: &str, value: &str) -> io::Result<()> {
    let width = terminal_width();
    let label: String = label.chars().take(width.saturating_sub(1)).collect();
    let value = escaped(value);
    // Budget up to two cells for each character. Keep the end visible while editing.
    let cell_budget = width.saturating_sub(label.len() + 1);
    let limit = if value.is_ascii() {
        cell_budget
    } else {
        cell_budget / 2
    };
    let start = value
        .char_indices()
        .rev()
        .nth(limit)
        .map_or(0, |(offset, ch)| offset + ch.len_utf8());
    queue!(output, MoveToColumn(0), Clear(ClearType::CurrentLine))?;
    write!(output, "{label}{}", &value[start..])?;
    output.flush()
}

pub(super) fn folder() -> io::Result<Option<PathBuf>> {
    prompt(|output| {
        let mut input = String::new();
        loop {
            redraw(output, "Folder path (Tab to complete): ", &input)?;
            match event::read()? {
                Event::Key(key) if accepts(key) => match key.code {
                    _ if cancelled(key) => break,
                    KeyCode::Enter if !input.trim().is_empty() => {
                        writeln!(output, "\r")?;
                        return Ok(Some(Path::new(&input).to_path_buf()));
                    }
                    KeyCode::Backspace => {
                        input.pop();
                    }
                    KeyCode::Tab => {
                        if let Some(path) =
                            complete(&input).filter(|path| path.len() <= MAX_PATH_INPUT)
                        {
                            input = path;
                        }
                    }
                    KeyCode::Char(ch)
                        if !ch.is_control()
                            && !key
                                .modifiers
                                .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
                            && input.len() + ch.len_utf8() <= MAX_PATH_INPUT =>
                    {
                        input.push(ch);
                    }
                    _ => {}
                },
                Event::Paste(text)
                    if !text.chars().any(char::is_control)
                        && text.len() <= MAX_PATH_INPUT - input.len() =>
                {
                    input.push_str(&text);
                }
                _ => {}
            }
        }
        writeln!(output, "\r")?;
        Ok(None)
    })
}

pub(super) fn choose(label: &str, choices: &[&str]) -> io::Result<Option<usize>> {
    prompt(|output| {
        let mut selected = 0;
        loop {
            let options = choices
                .iter()
                .enumerate()
                .map(|(i, choice)| {
                    if selected == i {
                        format!("[{choice}]")
                    } else {
                        (*choice).to_owned()
                    }
                })
                .collect::<Vec<_>>()
                .join(" / ");
            let width = terminal_width();
            if label.len() + options.len() + 3 > width {
                let compact = format!("[{}] {label}", choices[selected]);
                let visible: String = compact.chars().take(width.saturating_sub(1)).collect();
                redraw(output, "", &visible)?;
            } else {
                redraw(output, &format!("{label}: "), &options)?;
            }
            if let Event::Key(key) = event::read()?
                && accepts(key)
            {
                match key.code {
                    _ if cancelled(key) => {
                        writeln!(output, "\r")?;
                        return Ok(None);
                    }
                    KeyCode::Enter => {
                        writeln!(output, "\r")?;
                        return Ok(Some(selected));
                    }
                    KeyCode::Up | KeyCode::Left => {
                        selected = (selected + choices.len() - 1) % choices.len()
                    }
                    KeyCode::Down | KeyCode::Right => selected = (selected + 1) % choices.len(),
                    _ => {}
                }
            }
        }
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::tests::Scratch;
    use std::fs;

    #[test]
    fn completion_only_uses_directories_and_preserves_spaces() {
        let scratch = Scratch::new();
        let base = scratch.0.canonicalize().unwrap();
        fs::create_dir(base.join("album one")).unwrap();
        fs::write(base.join("album old"), b"file").unwrap();
        let typed = base.join("al");
        let expected = format!(
            "{}{}",
            base.join("album one").display(),
            std::path::MAIN_SEPARATOR
        );
        assert_eq!(complete(typed.to_str().unwrap()).unwrap(), expected);
    }

    #[test]
    fn only_enter_presses_commit_choices_and_controls_are_escaped() {
        assert!(accepts(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)));
        assert!(!accepts(KeyEvent::new_with_kind(
            KeyCode::Enter,
            KeyModifiers::NONE,
            KeyEventKind::Repeat
        )));
        assert!(!accepts(KeyEvent::new_with_kind(
            KeyCode::Left,
            KeyModifiers::NONE,
            KeyEventKind::Release
        )));
        assert_eq!(escaped("a\x1b\u{202e}"), "a\\u{1b}\\u{202e}");
    }
}
