use std::fs;
use std::path::PathBuf;

use clap::{Parser, Subcommand};
use pg_sql_migrate::baseline::{
    CaptureOptions, capture_baseline, to_canonical_json, write_baseline,
};
use pg_sql_migrate::{Mapping, inventory, to_canonical_inventory_json};

#[derive(Parser)]
#[command(
    name = "pg-sql-migrate",
    about = "Auditable pg-sql grammar migration tooling"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Print the complete legacy migration inventory as JSON. This command is read-only.
    Inventory {
        #[arg(default_value = ".")]
        root: PathBuf,
        /// Byte-compare stdout's canonical report with this file instead of printing it.
        #[arg(long)]
        check: Option<PathBuf>,
    },
    /// Capture or review the pinned PostgreSQL differential baseline.
    Baseline {
        #[command(subcommand)]
        command: BaselineCommand,
    },
}

#[derive(Subcommand)]
enum BaselineCommand {
    Capture {
        #[arg(long, default_value = "../recursa-old")]
        legacy_repository: PathBuf,
        #[arg(long, default_value = "vendor/postgres")]
        postgres_repository: PathBuf,
        #[arg(long, default_value = "baselines/postgresql-17.9.json")]
        output: PathBuf,
    },
    Review {
        #[arg(long, default_value = "../recursa-old")]
        legacy_repository: PathBuf,
        #[arg(long, default_value = "vendor/postgres")]
        postgres_repository: PathBuf,
        #[arg(long, default_value = "baselines/postgresql-17.9.json")]
        baseline: PathBuf,
    },
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    match Cli::parse().command {
        Command::Inventory { root, check } => {
            let report = inventory(&root, &Mapping::migration_contract())?;
            let canonical = to_canonical_inventory_json(&report)?;
            if let Some(path) = check {
                let expected = fs::read(&path)?;
                if expected != canonical.as_bytes() {
                    return Err(format!("inventory differs from {}", path.display()).into());
                }
                println!("inventory matches {}", path.display());
            } else {
                print!("{canonical}");
            }
        }
        Command::Baseline { command } => match command {
            BaselineCommand::Capture {
                legacy_repository,
                postgres_repository,
                output,
            } => {
                let options = CaptureOptions::new(legacy_repository, postgres_repository);
                let baseline = capture_baseline(&options)?;
                write_baseline(&output, &baseline)?;
                println!("wrote {}", output.display());
            }
            BaselineCommand::Review {
                legacy_repository,
                postgres_repository,
                baseline,
            } => {
                let expected = fs::read_to_string(&baseline)?;
                let options = CaptureOptions::new(legacy_repository, postgres_repository);
                let captured = capture_baseline(&options)?;
                let actual = to_canonical_json(&captured)?;
                if expected.as_bytes() != actual.as_bytes() {
                    return Err(format!(
                        "baseline differs from fresh capture: {}",
                        baseline.display()
                    )
                    .into());
                }
                println!("baseline matches fresh capture: {}", baseline.display());
            }
        },
    }
    Ok(())
}
