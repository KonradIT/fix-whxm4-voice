use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{CommandFactory, Parser, Subcommand};
use clap_complete::Shell;
use fix_whxm4_voice::bluetooth::{self, Headphones};
use fix_whxm4_voice::build::build;
use fix_whxm4_voice::device_info;
use fix_whxm4_voice::download::download;
use fix_whxm4_voice::generate;
use fix_whxm4_voice::language::Language;
use fix_whxm4_voice::mdr::MdrLink;
use fix_whxm4_voice::race::RaceLink;
use fix_whxm4_voice::source;
use fix_whxm4_voice::workflow;
use fix_whxm4_voice::workspace::Workspace;

/// Replace the voice prompts on Sony WH-1000XM4 headphones.
#[derive(Parser)]
#[command(version)]
struct Cli {
    /// Workspace directory (created if missing)
    #[arg(short = 'd', long = "dir", global = true, default_value = ".")]
    dir: PathBuf,

    /// Set up the workspace without asking, even in a directory holding other things, and let
    /// `generate` replace changes for the slots it generates
    #[arg(short = 'f', long = "force", global = true)]
    force: bool,

    /// Base language to download the originals in
    #[arg(long, global = true, value_enum, default_value_t = Language::English)]
    lang: Language,

    /// Language the headphones install the voice pack as, replacing that language's prompts.
    /// English cannot be replaced: the headphones always play their built-in English
    #[arg(long, global = true, value_enum, default_value_t = Language::German)]
    install_lang: Language,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Download the stock voice pack and save its clips in original/
    Download,
    /// Generate a theme of tone clips into changes/
    ///
    /// The theme is modern and calm: warm, soft tones that ring out like something struck. Power on
    /// and connecting rise in pitch, power off and disconnecting fall, connections start with a dot
    /// per device, a lost connection crackles, and a volume step is a short beep.
    ///
    /// The clips come from a bash script using ffmpeg, sounds/generate.sh in the project's
    /// repository on GitHub. The script also runs on its own: generate.sh <changes-dir>
    Generate,
    /// Build the modified voice pack in build/
    Build,
    /// Upload the built voice pack to the headphones over Bluetooth
    Upload {
        /// Go through every step except erasing and writing
        #[arg(long)]
        dry_run: bool,
    },
    /// Show what the headphones report about themselves, sending read-only queries only
    DeviceInfo {
        /// Log every command and reply as raw bytes
        #[arg(short, long)]
        verbose: bool,
    },
    /// Print a shell completion script to stdout
    Completions {
        /// Shell to print the script for
        #[arg(value_enum)]
        shell: Shell,
    },
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> io::Result<ExitCode> {
    match cli.command {
        None => {
            let ws = open_workspace(&cli.dir, cli.force)?;
            match workflow::prepare(&ws, cli.lang, source::fetch)? {
                Some(count) => eprintln!("saved {count} originals in {}", ws.original.display()),
                None => eprintln!(
                    "originals already in {}, skipping the download",
                    ws.original.display()
                ),
            }
            let upload = if cli.dir == Path::new(".") {
                "fix-whxm4-voice upload".to_string()
            } else {
                format!("fix-whxm4-voice -d {} upload", cli.dir.display())
            };
            eprintln!("\n{}", workflow::next_steps(&ws, &upload));
            Ok(ExitCode::SUCCESS)
        }
        Some(Command::Download) => {
            let ws = open_workspace(&cli.dir, cli.force)?;
            let count = download(&ws, cli.lang)?;
            eprintln!("saved {count} originals in {}", ws.original.display());
            Ok(ExitCode::SUCCESS)
        }
        Some(Command::Generate) => {
            let ws = open_workspace(&cli.dir, cli.force)?;
            let generated = generate::generate(&ws, &generate::script(), cli.force)?;
            eprintln!(
                "generated {} clips in {}: {}",
                generated.len(),
                ws.changes.display(),
                generated.join(", ")
            );
            Ok(ExitCode::SUCCESS)
        }
        Some(Command::Build) => {
            let ws = open_workspace(&cli.dir, cli.force)?;
            let path = build(&ws, cli.install_lang)?;
            eprintln!(
                "built and verified {}, to install as {}",
                path.display(),
                cli.install_lang.name()
            );
            Ok(ExitCode::SUCCESS)
        }
        Some(Command::Upload { dry_run: _ }) => {
            open_workspace(&cli.dir, cli.force)?;
            not_implemented("upload")
        }
        Some(Command::DeviceInfo { verbose }) => {
            let headphones = Headphones::find()?;
            eprintln!("found {} ({})", headphones.name, headphones.address);
            let mut mdr = MdrLink::new(headphones.open(bluetooth::MDR_UUID, "MDR", verbose)?);
            let mut race = RaceLink::new(headphones.open(bluetooth::RACE_UUID, "RACE", verbose)?);
            println!("{}", device_info::read(&mut mdr, &mut race)?);
            Ok(ExitCode::SUCCESS)
        }
        Some(Command::Completions { shell }) => {
            let mut cmd = Cli::command();
            let name = cmd.get_name().to_string();
            clap_complete::generate(shell, &mut cmd, name, &mut io::stdout());
            Ok(ExitCode::SUCCESS)
        }
    }
}

fn open_workspace(dir: &Path, force: bool) -> io::Result<Workspace> {
    Workspace::open(dir, force, confirm_not_dedicated)?
        .ok_or_else(|| io::Error::other("aborted, nothing was created"))
}

fn not_implemented(what: &str) -> io::Result<ExitCode> {
    eprintln!("{what} is not implemented yet");
    Ok(ExitCode::FAILURE)
}

fn confirm_not_dedicated(dir: &Path, others: &[String]) -> io::Result<bool> {
    let mut stderr = io::stderr().lock();
    writeln!(stderr, "{} already contains other things:", dir.display())?;
    for name in others {
        writeln!(stderr, "  {name}")?;
    }
    writeln!(
        stderr,
        "This tool should run in its own directory. It will create original/, changes/, build/ and stock/ here."
    )?;
    write!(stderr, "Continue anyway? [y/N] ")?;
    stderr.flush()?;

    let mut answer = String::new();
    io::stdin().lock().read_line(&mut answer)?;
    Ok(matches!(answer.trim().to_lowercase().as_str(), "y" | "yes"))
}
