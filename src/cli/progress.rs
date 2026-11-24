use crate::application::apply::{Phase, Progress};
use crossterm::cursor::MoveToColumn;
use crossterm::queue;
use crossterm::terminal::{self, Clear, ClearType};
use std::io::{self, Write};
use std::time::{Duration, Instant};

pub(super) struct Display<W> {
    output: W,
    last_draw: Option<(Phase, Instant)>,
    dirty: bool,
    failed: bool,
}

impl<W: Write> Display<W> {
    pub(super) fn new(output: W) -> Self {
        Self {
            output,
            last_draw: None,
            dirty: false,
            failed: false,
        }
    }

    pub(super) fn report(&mut self, progress: Progress) {
        if self.failed {
            return;
        }
        let now = Instant::now();
        if progress.completed != progress.total
            && self.last_draw.is_some_and(|(phase, at)| {
                phase == progress.phase && now.duration_since(at) < Duration::from_millis(75)
            })
        {
            return;
        }
        let cells = terminal::size().map_or(79, |(cols, _)| usize::from(cols).saturating_sub(1));
        let status = line(progress, cells);
        self.dirty = true;
        self.failed = self.draw(&status).is_err();
        self.last_draw = Some((progress.phase, now));
    }

    fn draw(&mut self, text: &str) -> io::Result<()> {
        queue!(self.output, MoveToColumn(0), Clear(ClearType::CurrentLine))?;
        self.output.write_all(text.as_bytes())?;
        self.output.flush()
    }

    pub(super) fn finish(&mut self) {
        if self.dirty {
            let _ = self.draw("");
        }
    }
}

fn line(progress: Progress, cells: usize) -> String {
    let label = match progress.phase {
        Phase::Staging => "Preparing files",
        Phase::Renaming => "Renaming",
        Phase::RemovingDuplicates => "Removing duplicates",
        Phase::CheckingDirectories => "Checking subfolders",
        Phase::Restoring => "Restore attempts",
    };
    let count = format!("{}/{}", progress.completed, progress.total);
    if count.len() > cells {
        return String::new();
    }
    let text = format!("{label} {count}");
    if text.len() > cells {
        return count;
    }
    let width = cells.saturating_sub(text.len() + 3).min(20);
    if width == 0 {
        return text;
    }
    let filled = (progress.completed.min(progress.total) as u128 * width as u128
        / progress.total.max(1) as u128) as usize;
    format!(
        "{text} [{}{}]",
        "#".repeat(filled),
        "-".repeat(width - filled)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rename(completed: usize) -> Progress {
        Progress {
            phase: Phase::Renaming,
            completed,
            total: 40,
        }
    }

    #[test]
    fn format_fits_the_terminal_without_hiding_counts_on_common_widths() {
        assert_eq!(line(rename(12), 16), "Renaming 12/40");
        assert_eq!(line(rename(12), 9), "12/40");
        assert!(line(rename(12), 3).is_empty());
        assert_eq!(line(rename(12), 33).len(), 33);
        assert!(line(rename(12), 60).contains("[######--------------]"));
    }

    #[test]
    fn failed_output_disables_further_updates() {
        struct Closed(usize);

        impl Write for Closed {
            fn write(&mut self, _: &[u8]) -> io::Result<usize> {
                self.0 += 1;
                Err(io::ErrorKind::BrokenPipe.into())
            }

            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }

        let mut display = Display::new(Closed(0));
        display.report(rename(0));
        let attempts = display.output.0;
        assert!(display.failed);
        display.report(rename(40));
        assert_eq!(display.output.0, attempts);
    }
}
