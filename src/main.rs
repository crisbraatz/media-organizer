mod application;
mod cli;
mod domain;
mod infrastructure;

fn main() -> std::process::ExitCode {
    cli::run(
        std::env::args_os().skip(1),
        &infrastructure::LocalMediaSource,
    )
}
