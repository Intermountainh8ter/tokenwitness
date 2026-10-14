//! tokenwitness CLI — scaffold.
//!
//! The initial release only knows how to diff two already-captured
//! renderings (loaded from JSON). The `capture` subcommand that drives real
//! stacks (llama.cpp over its logging, HF via `apply_chat_template`,
//! vLLM over its wire log, OpenAI-compatible endpoints that cooperate) is
//! TODO; `docs/PREREGISTRATION.md` locks the comparison method before any
//! real capture ships.

use clap::{Parser, Subcommand};
use tokenwitness::Rendering;

#[derive(Parser)]
#[command(
    name = "tokenwitness",
    version,
    about = "Chat-template byte-identity auditor"
)]
struct Cli {
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Diff two renderings captured as JSON and print the byte-level result.
    Diff {
        a: std::path::PathBuf,
        b: std::path::PathBuf,
    },
    /// Print the version banner and exit.
    Version,
}

fn load(path: &std::path::Path) -> anyhow::Result<Rendering> {
    let text = std::fs::read_to_string(path)?;
    Ok(serde_json::from_str(&text)?)
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Cmd::Diff { a, b } => {
            let ra = load(&a)?;
            let rb = load(&b)?;
            let d = tokenwitness::diff(&ra, &rb);
            println!("stacks:    {} vs {}", d.stack_a, d.stack_b);
            println!("len(a):    {}", d.len_a);
            println!("len(b):    {}", d.len_b);
            println!("sha(a):    {}", d.sha_a);
            println!("sha(b):    {}", d.sha_b);
            match d.first_diff {
                None if d.identical() => println!("verdict:   identical"),
                None => println!("verdict:   same prefix but len differs (len_a != len_b)"),
                Some(i) => println!("verdict:   differ at byte {i}"),
            }
        }
        Cmd::Version => println!("tokenwitness {}", env!("CARGO_PKG_VERSION")),
    }
    Ok(())
}
