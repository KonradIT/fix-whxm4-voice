use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{CommandFactory, Parser, Subcommand};
use clap_complete::Shell;
use fix_whxm4_voice::bluetooth::{self, Headphones};
use fix_whxm4_voice::build::{self, BUILT_PACK, BuiltPack, build};
use fix_whxm4_voice::device_info;
use fix_whxm4_voice::download::download;
use fix_whxm4_voice::generate;
use fix_whxm4_voice::language::Language;
use fix_whxm4_voice::mdr::MdrLink;
use fix_whxm4_voice::race::RaceLink;
use fix_whxm4_voice::source;
use fix_whxm4_voice::upload::{self, Mode, Operator, Outcome};
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
    /// `build` uses German unless this is given. `upload` keeps the language of the existing build,
    /// and rebuilds if this asks for another. English is allowed after a warning, but the headphones
    /// probably keep playing their built-in English
    #[arg(long, global = true, value_enum)]
    install_lang: Option<Language>,

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
        /// Switch the headphones' language update mode on, report the FOTA partition and what the
        /// upload would erase and write, and switch it off again. Nothing is erased, written,
        /// checked or committed
        #[arg(long)]
        dry_run: bool,
        /// Log every command and reply as raw bytes
        #[arg(short, long)]
        verbose: bool,
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
            let again = if cli.dir == Path::new(".") {
                "fix-whxm4-voice".to_string()
            } else {
                format!("fix-whxm4-voice -d {}", cli.dir.display())
            };
            let changes = workflow::change_names(&ws)?;
            if changes.is_empty() {
                eprintln!("\n{}", workflow::next_steps(&ws, &again));
                return Ok(ExitCode::SUCCESS);
            }
            eprintln!("\n{}", workflow::done_steps(&ws, &changes));
            let noun = if changes.len() == 1 {
                "change"
            } else {
                "changes"
            };
            let answer = ask(&format!("\nUpload {} {noun} now? [y/N] ", changes.len()))?;
            if !matches!(answer.to_lowercase().as_str(), "y" | "yes") {
                eprintln!("not uploaded; run `{again}` again when you are ready");
                return Ok(ExitCode::SUCCESS);
            }
            if !confirm_english(cli.install_lang)? {
                return Ok(ExitCode::FAILURE);
            }
            // The changes were just listed, so the build is not reported.
            let fresh = build::fresh(&ws, cli.install_lang)?;
            upload_pack(&fresh.pack, Mode::Upload, false)
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
            let install = cli.install_lang.unwrap_or(build::DEFAULT_INSTALL);
            if !confirm_english(Some(install))? {
                return Ok(ExitCode::FAILURE);
            }
            let path = build(&ws, install)?;
            let changes = workflow::change_names(&ws)?;
            eprintln!("{}", workflow::build_report(&path, &changes, true));
            Ok(ExitCode::SUCCESS)
        }
        Some(Command::Upload { dry_run, verbose }) => {
            let ws = open_workspace(&cli.dir, cli.force)?;
            if !confirm_english(cli.install_lang)? {
                return Ok(ExitCode::FAILURE);
            }
            let fresh = build::fresh(&ws, cli.install_lang)?;
            let changes = workflow::change_names(&ws)?;
            let path = ws.build.join(BUILT_PACK);
            let report = workflow::build_report(&path, &changes, fresh.rebuilt);
            eprintln!("{report}\n");
            let mode = if dry_run { Mode::DryRun } else { Mode::Upload };
            upload_pack(&fresh.pack, mode, verbose)
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

/// Uploads `pack` in `mode`. A real upload erases or writes nothing until the user has typed their
/// confirmation of the plan.
fn upload_pack(pack: &BuiltPack, mode: Mode, verbose: bool) -> io::Result<ExitCode> {
    // Refuses unless exactly one connected device offers Sony's control channel: the connections the
    // PC can see. The headphones' connections to other devices are not visible from here.
    let headphones = Headphones::find()?;
    eprintln!(
        "found {} ({}), the only headphones connected to this PC",
        headphones.name, headphones.address
    );
    let mut mdr = MdrLink::new(headphones.open(bluetooth::MDR_UUID, "MDR", verbose)?);
    let mut race = RaceLink::new(headphones.open(bluetooth::RACE_UUID, "RACE", verbose)?);
    let mut terminal = Terminal::default();
    let outcome = upload::run(&mut mdr, &mut race, pack, mode, &mut terminal);
    terminal.end_progress();
    match outcome? {
        Outcome::Planned(plan) => {
            println!("{plan}");
            eprintln!(
                "dry run: nothing was erased, written or committed, and language update mode is off again"
            );
            Ok(ExitCode::SUCCESS)
        }
        Outcome::Declined(_) => {
            eprintln!(
                "cancelled: nothing was erased or written, and language update mode is off again"
            );
            Ok(ExitCode::FAILURE)
        }
        Outcome::Committed(plan) => {
            let language = device_info::language_name(plan.install.code() as u8);
            eprintln!(
                "done: the headphones passed their checksum check of the pack and are restarting to install \
                 it as {language}. Once they are back, `device-info` should report {language} as the \
                 current language."
            );
            Ok(ExitCode::SUCCESS)
        }
    }
}

/// The user at the terminal: confirms the plan by typing, and watches a progress bar.
#[derive(Default)]
struct Terminal {
    /// The percentage the progress bar shows, while one is on screen.
    shown: Option<usize>,
}

/// The word the user types to confirm an upload.
const CONFIRMATION: &str = "upload";
/// The width of the progress bar, in characters.
const BAR_WIDTH: usize = 40;

impl Terminal {
    /// Ends the line of the progress bar, if one is on screen.
    fn end_progress(&mut self) {
        if self.shown.take().is_some() {
            eprintln!();
        }
    }
}

impl Operator for Terminal {
    fn confirm(&mut self, plan: &upload::Plan) -> io::Result<bool> {
        eprintln!("{plan}\n");
        eprintln!(
            "During the upload, leave the headphones alone: don't press their buttons, \
             and don't open the Sony app."
        );
        let answer = ask(&format!(
            "Type `{CONFIRMATION}` to erase and write this range of the headphones' flash, anything else \
             to cancel: "
        ))?;
        Ok(answer == CONFIRMATION)
    }

    fn progress(&mut self, done: usize, total: usize) {
        let percent = done * 100 / total;
        if self.shown == Some(percent) {
            return;
        }
        self.shown = Some(percent);
        let filled = done * BAR_WIDTH / total;
        eprint!(
            "\r[{}{}] {percent:3}% ({done}/{total})",
            "█".repeat(filled),
            "░".repeat(BAR_WIDTH - filled)
        );
        let _ = io::stderr().flush();
    }
}

/// Shows `prompt` on stderr and returns the line the user typed, trimmed; empty at end of input.
/// An empty line follows the answer, setting it apart from what comes next.
fn ask(prompt: &str) -> io::Result<String> {
    let mut stderr = io::stderr().lock();
    write!(stderr, "{prompt}")?;
    stderr.flush()?;
    let mut answer = String::new();
    io::stdin().lock().read_line(&mut answer)?;
    // At the end of input, nothing ended the prompt's line.
    if !answer.ends_with('\n') {
        writeln!(stderr)?;
    }
    writeln!(stderr)?;
    Ok(answer.trim().to_string())
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
    drop(stderr);
    let answer = ask("Continue anyway? [y/N] ")?;
    Ok(matches!(answer.to_lowercase().as_str(), "y" | "yes"))
}

/// Asks before building as English, which the headphones probably ignore; `true` for any other
/// language, or none.
fn confirm_english(install: Option<Language>) -> io::Result<bool> {
    if install != Some(Language::English) {
        return Ok(true);
    }
    eprintln!(
        "warning: English is built into the headphones. A voice pack installed as English probably has no \
         effect: they keep playing the built-in English. German, the default, or another language works."
    );
    let answer = ask("Build the voice pack as English anyway? [y/N] ")?;
    if matches!(answer.to_lowercase().as_str(), "y" | "yes") {
        return Ok(true);
    }
    eprintln!("cancelled: nothing was built");
    Ok(false)
}
