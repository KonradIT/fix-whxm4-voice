use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use clap::error::ErrorKind;
use clap::parser::ValueSource;
use clap::{Args, CommandFactory, FromArgMatches, Parser, Subcommand};
use clap_complete::Shell;
use fix_whxm4_voice::bluetooth::{self, Headphones};
use fix_whxm4_voice::build::{self, BUILT_PACK, BuiltPack, build};
use fix_whxm4_voice::changes;
use fix_whxm4_voice::device_info;
use fix_whxm4_voice::download::{self, download};
use fix_whxm4_voice::generate::{self, Script};
use fix_whxm4_voice::language::Language;
use fix_whxm4_voice::mdr::MdrLink;
use fix_whxm4_voice::race::RaceLink;
use fix_whxm4_voice::source;
use fix_whxm4_voice::upload::{self, Mode, Operator, Outcome};
use fix_whxm4_voice::workflow;
use fix_whxm4_voice::workspace::Workspace;

/// Replace the voice prompts on Sony WH-1000XM4 headphones.
///
/// Without a command, it downloads the originals if there are none yet, and explains the next steps.
/// Once there are changes, it lists them and offers to build the voice pack and upload it.
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

    #[command(flatten)]
    base: BaseLang,

    #[command(flatten)]
    install: InstallLang,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Args)]
struct BaseLang {
    /// Base language to download the originals in
    #[arg(long, value_enum, default_value_t = Language::English)]
    lang: Language,
}

#[derive(Args)]
struct InstallLang {
    /// Language the headphones install the voice pack as, replacing that language's prompts
    ///
    /// Without this, `build` builds German, and an upload keeps the language of the existing build,
    /// or builds German if there is none. English is allowed after a warning, but the headphones
    /// probably keep playing their built-in English.
    #[arg(long, value_enum)]
    install_lang: Option<Language>,
}

#[derive(Subcommand)]
enum Command {
    /// Download the stock voice pack and save its clips in original/
    Download {
        #[command(flatten)]
        base: BaseLang,
    },
    /// Generate a theme of tone clips into changes/
    ///
    /// The theme is modern and calm: warm, soft tones that ring out like something struck. Power on
    /// and connecting rise in pitch, power off and disconnecting fall, connections start with a dot
    /// per device, a lost connection crackles, and a volume step is a short beep.
    ///
    /// The clips come from a bash script using ffmpeg, sounds/generate.sh in the project's
    /// repository on GitHub. The script also runs on its own: generate.sh <changes-dir>
    ///
    /// It also offers to shorten the battery prompts: of "battery, about seventy percent" only
    /// "seventy" is kept, of "battery, fully charged" only "full". These clips are cut from the
    /// English originals by sounds/battery.sh, which also runs on its own: battery.sh <changes-dir>
    /// <original-dir>. Without originals, it offers to download the English ones first. With
    /// originals in another language it says so and skips them.
    Generate {
        /// Also extract the battery clips, without asking. Downloads the English originals if there
        /// are none yet
        #[arg(long, conflicts_with = "no_battery")]
        battery: bool,
        /// Generate the tones only, without asking about the battery clips
        #[arg(long)]
        no_battery: bool,
        #[command(flatten)]
        base: BaseLang,
    },
    /// Build the modified voice pack in build/
    Build {
        #[command(flatten)]
        install: InstallLang,
    },
    /// Build the voice pack if it is missing or out of date, and upload it to the headphones over
    /// Bluetooth
    Upload {
        /// Switch the headphones' language update mode on, report the FOTA partition and what the
        /// upload would erase and write, and switch it off again. Nothing is erased, written,
        /// checked or committed
        #[arg(long)]
        dry_run: bool,
        /// Log every command and reply as raw bytes
        #[arg(short, long)]
        verbose: bool,
        #[command(flatten)]
        install: InstallLang,
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

/// What Ctrl-C does: [`QUIT`] at once, or while an upload is [`WATCHING`] for it, ask the upload to
/// stop, which makes it [`STOPPING`].
static CTRL_C: AtomicU8 = AtomicU8::new(QUIT);
const QUIT: u8 = 0;
const WATCHING: u8 = 1;
const STOPPING: u8 = 2;
/// The exit code of a program stopped by Ctrl-C, as shells report it.
const INTERRUPTED: u8 = 130;

fn main() -> ExitCode {
    let installed = ctrlc::set_handler(|| {
        if CTRL_C
            .compare_exchange(WATCHING, STOPPING, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
        {
            eprintln!(
                "\nstopping after the current command\n\
                 Press Ctrl-C again to quit at once, which may leave language update mode on."
            );
        } else {
            std::process::exit(INTERRUPTED.into());
        }
    });
    if let Err(e) = installed {
        eprintln!("error: cannot handle Ctrl-C: {e}");
        return ExitCode::FAILURE;
    }
    match run(parse()) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

/// Parses the command line, refusing a language given before a command: it would be the default
/// command's, and silently ignored.
fn parse() -> Cli {
    let mut cmd = Cli::command();
    let matches = cmd.get_matches_mut();
    if let Some(command) = matches.subcommand_name() {
        for (id, flag) in [("lang", "--lang"), ("install_lang", "--install-lang")] {
            if matches.value_source(id) == Some(ValueSource::CommandLine) {
                cmd.error(
                    ErrorKind::ArgumentConflict,
                    format!("{flag} goes after the command: {command} {flag} <...>"),
                )
                .exit();
            }
        }
    }
    Cli::from_arg_matches(&matches).unwrap_or_else(|e| e.exit())
}

fn run(cli: Cli) -> io::Result<ExitCode> {
    match cli.command {
        None => {
            let install = cli.install;
            let ws = open_workspace(&cli.dir, cli.force)?;
            match workflow::prepare(&ws, cli.base.lang, source::fetch)? {
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
            eprintln!("{}", workflow::change_list(&ws, &changes));
            // Changes that match no slot fail now rather than after the user agreed to upload.
            changes::check(&ws.changes)?;
            let noun = if changes.len() == 1 {
                "change"
            } else {
                "changes"
            };
            let answer = ask(&format!("\nUpload {} {noun} now? [y/N] ", changes.len()))?;
            if !matches!(answer.to_lowercase().as_str(), "y" | "yes") {
                eprintln!("not uploaded\nRun `{again}` again when you are ready.");
                return Ok(ExitCode::SUCCESS);
            }
            if !confirm_english(install.install_lang)? {
                return Ok(ExitCode::FAILURE);
            }
            // The changes were just listed, so the build is not reported.
            let fresh = build::fresh(&ws, install.install_lang)?;
            upload_pack(&fresh.pack, Mode::Upload, false)
        }
        Some(Command::Download { base }) => {
            let ws = open_workspace(&cli.dir, cli.force)?;
            let count = download(&ws, base.lang)?;
            eprintln!("saved {count} originals in {}", ws.original.display());
            Ok(ExitCode::SUCCESS)
        }
        Some(Command::Generate {
            battery,
            no_battery,
            base,
        }) => {
            let ws = open_workspace(&cli.dir, cli.force)?;
            let flag = match (battery, no_battery) {
                (true, _) => Some(true),
                (_, true) => Some(false),
                _ => None,
            };
            // The tones first, so the battery question comes after they are done.
            generate_clips(&ws, Script::sounds(), cli.force)?;
            if extract_battery(&ws, flag, base.lang)? {
                generate_clips(&ws, Script::battery(&ws), cli.force)?;
            }
            Ok(ExitCode::SUCCESS)
        }
        Some(Command::Build { install }) => {
            let ws = open_workspace(&cli.dir, cli.force)?;
            let install = install.install_lang.unwrap_or(build::DEFAULT_INSTALL);
            if !confirm_english(Some(install))? {
                return Ok(ExitCode::FAILURE);
            }
            let path = build(&ws, install)?;
            let changes = workflow::change_names(&ws)?;
            eprintln!("{}", workflow::build_report(&path, &changes, true));
            Ok(ExitCode::SUCCESS)
        }
        Some(Command::Upload {
            dry_run,
            verbose,
            install,
        }) => {
            let ws = open_workspace(&cli.dir, cli.force)?;
            if !confirm_english(install.install_lang)? {
                return Ok(ExitCode::FAILURE);
            }
            let fresh = build::fresh(&ws, install.install_lang)?;
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

/// Runs `script` into `changes/` and reports the clips it generated.
fn generate_clips(ws: &Workspace, script: Script, force: bool) -> io::Result<()> {
    let generated = generate::generate(ws, &[script], force)?;
    eprintln!(
        "generated {} clips in {}:{}",
        generated.len(),
        ws.changes.display(),
        generated
            .iter()
            .map(|name| format!("\n  {name}"))
            .collect::<String>()
    );
    Ok(())
}

/// Whether `generate` also extracts the battery clips: `flag` if given (`--battery` or
/// `--no-battery`), or else the user's answer. They are cut from English originals only: with
/// originals in another language, this says so and skips them. Without originals, it downloads the
/// English ones first, unless `lang`, the base language to download, is another.
fn extract_battery(ws: &Workspace, flag: Option<bool>, lang: Language) -> io::Result<bool> {
    if flag == Some(false) {
        return Ok(false);
    }
    let base = download::base_language(ws)?;
    if let Some(other) = base.or(Some(lang)).filter(|l| *l != Language::English) {
        let which = if base.is_some() {
            "the originals are"
        } else {
            "--lang asks for"
        };
        eprintln!(
            "skipping the battery clips: they can only be cut from English originals, and {which} {}",
            other.name()
        );
        return Ok(false);
    }
    let question = if base.is_some() {
        "Short battery clips (\"seventy\" instead of \"battery, about seventy percent\") can be cut from \
         the English originals.\nCut them? [y/N] "
    } else {
        "There are no originals yet. Short battery clips (\"seventy\" instead of \"battery, about seventy \
         percent\") can be cut from the English originals.\nDownload them and cut the clips? [y/N] "
    };
    let extract = match flag {
        Some(extract) => extract,
        None => matches!(ask(question)?.to_lowercase().as_str(), "y" | "yes"),
    };
    if extract && base.is_none() {
        let count = download(ws, Language::English)?;
        eprintln!("saved {count} originals in {}", ws.original.display());
    }
    Ok(extract)
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
    CTRL_C.store(WATCHING, Ordering::SeqCst);
    let outcome = upload::run(&mut mdr, &mut race, pack, mode, &mut terminal);
    CTRL_C.store(QUIT, Ordering::SeqCst);
    terminal.end_progress();
    let outcome = match outcome {
        Err(e) if e.kind() == io::ErrorKind::Interrupted => {
            eprintln!("{e}");
            return Ok(ExitCode::from(INTERRUPTED));
        }
        outcome => outcome?,
    };
    match outcome {
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
                 it as {language}.\n\
                 Once they are back, `device-info` should report {language} as the current language."
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
        let answer = ask_or_stop(&format!(
            "Type `{CONFIRMATION}` to erase and write this range of the headphones' flash.\n\
             Anything else cancels: "
        ))?;
        Ok(answer.as_deref() == Some(CONFIRMATION))
    }

    fn interrupted(&mut self) -> bool {
        CTRL_C.load(Ordering::SeqCst) == STOPPING
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

/// Like [`ask`], but returns `None` as soon as Ctrl-C asks the upload to stop, rather than waiting
/// for a line the user is not going to type.
fn ask_or_stop(prompt: &str) -> io::Result<Option<String>> {
    let mut stderr = io::stderr().lock();
    write!(stderr, "{prompt}")?;
    stderr.flush()?;
    drop(stderr);
    let (send, receive) = mpsc::channel();
    // Left blocked on stdin if Ctrl-C comes first; the program ends soon after.
    thread::spawn(move || {
        let mut answer = String::new();
        let read = io::stdin().lock().read_line(&mut answer).map(|_| answer);
        let _ = send.send(read);
    });
    loop {
        if CTRL_C.load(Ordering::SeqCst) == STOPPING {
            return Ok(None);
        }
        match receive.recv_timeout(Duration::from_millis(100)) {
            Ok(answer) => {
                let answer = answer?;
                // At the end of input, nothing ended the prompt's line.
                if !answer.ends_with('\n') {
                    eprintln!();
                }
                eprintln!();
                return Ok(Some(answer.trim().to_string()));
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => return Ok(None),
        }
    }
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
         effect: they keep playing the built-in English.\n\
         German, the default, or another language works."
    );
    let answer = ask("Build the voice pack as English anyway? [y/N] ")?;
    if matches!(answer.to_lowercase().as_str(), "y" | "yes") {
        return Ok(true);
    }
    eprintln!("cancelled: nothing was built");
    Ok(false)
}
