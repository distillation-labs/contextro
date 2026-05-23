use anyhow::{anyhow, Context, Result};

use super::DEFAULT_TASKS;

pub(super) struct CliArgs {
    pub(super) codebase: String,
    pub(super) output_dir: String,
    pub(super) tasks: usize,
}

pub(super) fn parse_args() -> Result<CliArgs> {
    let mut codebase: Option<String> = None;
    let mut output_dir: Option<String> = None;
    let mut tasks = DEFAULT_TASKS;

    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--codebase" => {
                codebase = Some(
                    args.next()
                        .ok_or_else(|| anyhow!("missing value for --codebase"))?,
                );
            }
            "--output-dir" => {
                output_dir = Some(
                    args.next()
                        .ok_or_else(|| anyhow!("missing value for --output-dir"))?,
                );
            }
            "--tasks" => {
                let value = args
                    .next()
                    .ok_or_else(|| anyhow!("missing value for --tasks"))?;
                tasks = value
                    .parse::<usize>()
                    .with_context(|| format!("invalid --tasks value '{value}'"))?;
            }
            "--help" | "-h" => {
                println!(
                    "contextro-study --codebase PATH --output-dir DIR [--tasks N]\n\
                     Options:\n\
                     - --codebase PATH   path to the codebase to study (required)\n\
                     - --output-dir DIR  directory to write study results (required)\n\
                     - --tasks N         number of tasks to generate (default: {})",
                    DEFAULT_TASKS
                );
                std::process::exit(0);
            }
            other => return Err(anyhow!("unknown argument '{other}'")),
        }
    }

    let codebase = codebase.ok_or_else(|| anyhow!("--codebase PATH is required"))?;
    let output_dir = output_dir.ok_or_else(|| anyhow!("--output-dir DIR is required"))?;

    if tasks < 100 {
        return Err(anyhow!("--tasks must be at least 100"));
    }

    Ok(CliArgs {
        codebase,
        output_dir,
        tasks,
    })
}
