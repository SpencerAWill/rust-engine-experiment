#![forbid(unsafe_code)]
// example binary, not simulation code
#![allow(clippy::print_stdout, clippy::print_stderr)]
//! Command-line wrapper around [`fixed_step_sim::run`].
//!
//! ```text
//! cargo run -p fixed_step_sim --release -- [--seed N] [--jitter-seed N] [--ticks N] [--particles N]
//! ```
//!
//! Prints `tick=… live=… capacity=… checksum=0x…`.

use std::process::ExitCode;

use fixed_step_sim::{SimConfig, run};

fn parse(args: &[String]) -> Result<SimConfig, String> {
    let mut config = SimConfig::default();
    let mut it = args.iter();
    while let Some(flag) = it.next() {
        let mut value = |name: &str| -> Result<u64, String> {
            let text = it
                .next()
                .ok_or_else(|| format!("`{name}` needs a number"))?;
            text.parse()
                .map_err(|_| format!("`{name}` needs a number, got `{text}`"))
        };
        match flag.as_str() {
            "--seed" => config.seed = value("--seed")?,
            "--jitter-seed" => config.jitter_seed = value("--jitter-seed")?,
            "--ticks" => config.ticks = value("--ticks")?,
            "--particles" => {
                config.particles = usize::try_from(value("--particles")?)
                    .map_err(|_| "`--particles` is too large".to_string())?;
            }
            other => return Err(format!("unknown argument `{other}`")),
        }
    }
    Ok(config)
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match parse(&args) {
        Ok(config) => {
            let summary = run(&config);
            println!(
                "tick={} live={} capacity={} checksum={:#018x}",
                summary.tick, summary.live, summary.capacity, summary.checksum
            );
            ExitCode::SUCCESS
        }
        Err(message) => {
            eprintln!("fixed_step_sim: {message}");
            ExitCode::FAILURE
        }
    }
}
