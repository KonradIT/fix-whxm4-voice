//! Shell completion scripts, written from the clap command tree.
//!
//! clap_complete offers options the command line no longer allows, such as anything after
//! `--version`, and its bash script misses every subcommand of a program with a hyphenated name. So
//! the scripts are written here instead. Every shell follows the same rules:
//!
//! - A subcommand is offered only right after its parent command: a command's options go after it.
//! - After an option that ends the command line, such as `--help` or `--version`, nothing is offered.
//! - An option is not offered again once given, in either form, nor any option it conflicts with.
//! - A value is completed from its possible values, as a directory, or not at all.

use std::fmt::Write;

use clap::{Arg, ArgAction, Command, ValueEnum, ValueHint};

/// A shell to write a completion script for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum Shell {
    Bash,
    Zsh,
    Fish,
}

/// The completion script for `shell`, completing `cmd` and its subcommands.
pub fn script(cmd: &Command, shell: Shell) -> String {
    let mut cmd = cmd.clone();
    cmd.build();
    let spec = Spec::of(&cmd, String::new());
    let bin = cmd.get_bin_name().unwrap_or(cmd.get_name()).to_string();
    match shell {
        Shell::Bash => bash(&bin, &spec),
        Shell::Zsh => zsh(&bin, &spec),
        Shell::Fish => fish(&bin, &spec),
    }
}

/// What a command accepts, as far as completion cares.
struct Spec {
    /// The subcommand names leading here, joined by `/`; empty for the program itself.
    path: String,
    about: String,
    options: Vec<Opt>,
    /// The value of each positional argument, in order.
    positionals: Vec<Value>,
    subcommands: Vec<Spec>,
}

struct Opt {
    shorts: Vec<char>,
    longs: Vec<String>,
    help: String,
    value: Value,
    /// Whether nothing may follow, as after `--help`.
    ends: bool,
    /// The indexes in [`Spec::options`] of the options this one conflicts with, in either direction.
    conflicts: Vec<usize>,
}

#[derive(Clone)]
enum Value {
    /// A flag, which takes no value.
    None,
    /// A value the script cannot suggest.
    Any,
    Directory,
    Choices(Vec<String>),
}

impl Spec {
    fn of(cmd: &Command, path: String) -> Self {
        let args: Vec<&Arg> = cmd
            .get_arguments()
            .filter(|arg| !arg.is_hide_set() && !arg.is_positional())
            .collect();
        let options = args
            .iter()
            .map(|arg| {
                let conflicts = args
                    .iter()
                    .enumerate()
                    .filter(|(_, other)| {
                        let id = other.get_id();
                        cmd.get_arg_conflicts_with(arg)
                            .iter()
                            .any(|c| c.get_id() == id)
                            || cmd
                                .get_arg_conflicts_with(other)
                                .iter()
                                .any(|c| c.get_id() == arg.get_id())
                    })
                    .map(|(index, _)| index)
                    .collect();
                Opt {
                    shorts: arg.get_short_and_visible_aliases().unwrap_or_default(),
                    longs: arg
                        .get_long_and_visible_aliases()
                        .unwrap_or_default()
                        .into_iter()
                        .map(String::from)
                        .collect(),
                    help: first_line(arg.get_help()),
                    value: value(arg),
                    ends: matches!(
                        arg.get_action(),
                        ArgAction::Help
                            | ArgAction::HelpShort
                            | ArgAction::HelpLong
                            | ArgAction::Version
                    ),
                    conflicts,
                }
            })
            .collect();
        let positionals = cmd
            .get_positionals()
            .filter(|arg| !arg.is_hide_set())
            .map(value)
            .collect();
        let subcommands = cmd
            .get_subcommands()
            .filter(|sub| !sub.is_hide_set())
            .map(|sub| {
                let path = if path.is_empty() {
                    sub.get_name().to_string()
                } else {
                    format!("{path}/{}", sub.get_name())
                };
                Spec::of(sub, path)
            })
            .collect();
        Spec {
            path,
            about: first_line(cmd.get_about()),
            options,
            positionals,
            subcommands,
        }
    }

    /// This command and every command below it.
    fn all(&self) -> Vec<&Spec> {
        let mut all = vec![self];
        for sub in &self.subcommands {
            all.extend(sub.all());
        }
        all
    }

    /// The name of this command, the last part of its path.
    fn name(&self) -> &str {
        self.path.rsplit('/').next().unwrap_or_default()
    }
}

impl Opt {
    /// Every form of the option: `-d`, `--dir`.
    fn forms(&self) -> Vec<String> {
        let shorts = self.shorts.iter().map(|s| format!("-{s}"));
        shorts
            .chain(self.longs.iter().map(|l| format!("--{l}")))
            .collect()
    }
}

fn value(arg: &Arg) -> Value {
    if !arg.get_action().takes_values() {
        return Value::None;
    }
    let choices: Vec<String> = arg
        .get_possible_values()
        .iter()
        .filter(|v| !v.is_hide_set())
        .map(|v| v.get_name().to_string())
        .collect();
    if !choices.is_empty() {
        Value::Choices(choices)
    } else if arg.get_value_hint() == ValueHint::DirPath {
        Value::Directory
    } else {
        Value::Any
    }
}

fn first_line(text: Option<&clap::builder::StyledStr>) -> String {
    let text = text.map(ToString::to_string).unwrap_or_default();
    text.lines()
        .next()
        .unwrap_or_default()
        .trim_end_matches('.')
        .to_string()
}

/// `name` made into a shell identifier: `fix-whxm4-voice` becomes `fix_whxm4_voice`.
fn ident(name: &str) -> String {
    name.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect()
}

/// `text` in single quotes, for bash and zsh.
fn quoted(text: &str) -> String {
    format!("'{}'", text.replace('\'', r"'\''"))
}

fn bash(bin: &str, spec: &Spec) -> String {
    let f = format!("_{}", ident(bin));
    let mut specs = String::new();
    for cmd in spec.all() {
        let list = |items: Vec<String>| {
            items
                .iter()
                .map(|i| quoted(i))
                .collect::<Vec<_>>()
                .join(" ")
        };
        let kinds = |values: Vec<&Value>, ends: Vec<bool>| {
            values
                .iter()
                .zip(ends)
                .map(|(value, ends)| match (value, ends) {
                    (_, true) => "end".to_string(),
                    (Value::None, _) => "flag".to_string(),
                    (Value::Any, _) => "any".to_string(),
                    (Value::Directory, _) => "dir".to_string(),
                    (Value::Choices(c), _) => format!("choice:{}", c.join(" ")),
                })
                .collect::<Vec<_>>()
        };
        let opts = &cmd.options;
        writeln!(specs, "        {})", quoted(&cmd.path)).unwrap();
        let subs = cmd
            .subcommands
            .iter()
            .map(|s| s.name().to_string())
            .collect();
        writeln!(specs, "            subs=({})", list(subs)).unwrap();
        writeln!(
            specs,
            "            opts=({})",
            list(opts.iter().map(|o| o.forms().join(" ")).collect())
        )
        .unwrap();
        let option_kinds = kinds(
            opts.iter().map(|o| &o.value).collect(),
            opts.iter().map(|o| o.ends).collect(),
        );
        writeln!(specs, "            kinds=({})", list(option_kinds)).unwrap();
        let conflicts = opts
            .iter()
            .map(|o| {
                o.conflicts
                    .iter()
                    .map(|c| c.to_string())
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .collect();
        writeln!(specs, "            conflicts=({})", list(conflicts)).unwrap();
        let positional_kinds = kinds(
            cmd.positionals.iter().collect(),
            vec![false; cmd.positionals.len()],
        );
        writeln!(
            specs,
            "            positionals=({})",
            list(positional_kinds)
        )
        .unwrap();
        writeln!(specs, "            ;;").unwrap();
    }
    format!(
        r#"# bash completion for {bin}

# Sets subs, opts, kinds, conflicts and positionals for the command at path $1.
{f}_spec() {{
    case $1 in
{specs}    esac
}}

# Completes $cur as a value of kind $1.
{f}_value() {{
    case $1 in
        choice:*) COMPREPLY=($(compgen -W "${{1#choice:}}" -- "$cur")) ;;
        dir)
            compopt -o filenames 2>/dev/null
            COMPREPLY=($(compgen -d -- "$cur"))
            ;;
    esac
}}

{f}() {{
    local cur=${{COMP_WORDS[COMP_CWORD]}}
    local -a subs opts kinds conflicts positionals used
    local path="" i=1 word k u found pending="" count=0 candidates=""
    {f}_spec "$path"
    # A subcommand counts only right after its parent.
    while (( i < COMP_CWORD )); do
        word=${{COMP_WORDS[i]}}
        found=""
        for k in "${{subs[@]}}"; do [[ $k == "$word" ]] && found=$k; done
        [[ -z $found ]] && break
        path=${{path:+$path/}}$found
        {f}_spec "$path"
        i=$((i + 1))
    done
    for (( ; i < COMP_CWORD; i++ )); do
        word=${{COMP_WORDS[i]}}
        if [[ -n $pending ]]; then
            # Bash splits --dir=x into --dir, = and x.
            [[ $word == = ]] && continue
            pending=""
            continue
        fi
        if [[ $word == -* ]]; then
            for k in "${{!opts[@]}}"; do
                if [[ " ${{opts[k]}} " == *" $word "* ]]; then
                    used+=("$k")
                    case ${{kinds[k]}} in
                        end) return 0 ;;
                        flag) ;;
                        *) pending=${{kinds[k]}} ;;
                    esac
                fi
            done
        else
            count=$((count + 1))
        fi
    done
    if [[ -n $pending ]]; then
        [[ $cur == = ]] && cur=""
        {f}_value "$pending"
        return 0
    fi
    if [[ $cur == -* ]]; then
        for k in "${{!opts[@]}}"; do
            found=""
            for u in "${{used[@]}}"; do
                [[ $u == "$k" || " ${{conflicts[k]}} " == *" $u "* ]] && found=1
            done
            [[ -z $found ]] && candidates+=" ${{opts[k]}}"
        done
        COMPREPLY=($(compgen -W "$candidates" -- "$cur"))
    elif (( count == 0 && ${{#used[@]}} == 0 && ${{#subs[@]}} > 0 )); then
        COMPREPLY=($(compgen -W "${{subs[*]}}" -- "$cur"))
    elif (( count < ${{#positionals[@]}} )); then
        {f}_value "${{positionals[count]}}"
    fi
}}

complete -F {f} {bin}
"#
    )
}

fn zsh(bin: &str, spec: &Spec) -> String {
    let mut script = format!("#compdef {bin}\n");
    for cmd in spec.all() {
        script.push('\n');
        script.push_str(&zsh_function(bin, cmd));
    }
    write!(
        script,
        r#"
if [[ $zsh_eval_context[-1] == loadautofunc ]]; then
    _{bin} "$@"
else
    compdef _{bin} {bin}
fi
"#
    )
    .unwrap();
    script
}

/// The zsh function completing `cmd`, named after its path.
fn zsh_function(bin: &str, cmd: &Spec) -> String {
    let name = zsh_name(bin, &cmd.path);
    let mut specs = Vec::new();
    for opt in &cmd.options {
        let excluded = if opt.ends {
            "- : *".to_string()
        } else {
            let conflicts = opt.conflicts.iter().flat_map(|&c| cmd.options[c].forms());
            opt.forms()
                .into_iter()
                .chain(conflicts)
                .collect::<Vec<_>>()
                .join(" ")
        };
        let help = opt
            .help
            .replace('\\', r"\\")
            .replace('[', r"\[")
            .replace(']', r"\]")
            .replace(':', r"\:");
        let action = match &opt.value {
            Value::None => String::new(),
            other => format!(":{}", zsh_action(other)),
        };
        let forms: Vec<String> = opt
            .shorts
            .iter()
            .map(|s| format!("-{s}{}", if opt.value.takes() { "+" } else { "" }))
            .chain(
                opt.longs
                    .iter()
                    .map(|l| format!("--{l}{}", if opt.value.takes() { "=" } else { "" })),
            )
            .collect();
        let forms = if forms.len() == 1 {
            forms[0].clone()
        } else {
            format!("{{{}}}", forms.join(","))
        };
        specs.push(format!(
            "{}{forms}{}",
            quoted(&format!("({excluded})")),
            quoted(&format!("[{help}]{action}"))
        ));
    }
    for (index, value) in cmd.positionals.iter().enumerate() {
        specs.push(quoted(&format!("{}:{}", index + 1, zsh_action(value))));
    }
    if cmd.subcommands.is_empty() {
        let specs: String = specs.iter().map(|s| format!(" \\\n        {s}")).collect();
        return format!("{name}() {{\n    _arguments -s -S :{specs}\n}}\n");
    }
    specs.push(quoted("1: :->command"));
    specs.push(quoted("*:: :->args"));
    let specs: String = specs.iter().map(|s| format!(" \\\n        {s}")).collect();
    let commands: String = cmd
        .subcommands
        .iter()
        .map(|sub| {
            format!(
                "\n                {}",
                quoted(&format!("{}:{}", sub.name(), sub.about))
            )
        })
        .collect();
    let dispatch: String = cmd
        .subcommands
        .iter()
        .map(|sub| {
            format!(
                "\n                ({}) {} && ret=0 ;;",
                sub.name(),
                zsh_name(bin, &sub.path)
            )
        })
        .collect();
    // A subcommand counts only right after its parent, so not after an option.
    format!(
        r#"{name}() {{
    local curcontext="$curcontext" state state_descr line ret=1
    typeset -A opt_args
    _arguments -s -S -C -A '-*' :{specs} && ret=0
    (( ${{#opt_args}} )) && return ret
    case $state in
        (command)
            local -a commands=({commands}
            )
            _describe -t commands command commands && ret=0
            ;;
        (args)
            curcontext="${{curcontext%:*:*}}:{bin}-$line[1]:"
            case $line[1] in{dispatch}
            esac
            ;;
    esac
    return ret
}}
"#
    )
}

fn zsh_name(bin: &str, path: &str) -> String {
    if path.is_empty() {
        format!("_{bin}")
    } else {
        format!("_{bin}_{}", ident(path))
    }
}

fn zsh_action(value: &Value) -> String {
    match value {
        Value::None | Value::Any => " ".to_string(),
        Value::Directory => "directory:_files -/".to_string(),
        Value::Choices(choices) => format!("value:({})", choices.join(" ")),
    }
}

impl Value {
    fn takes(&self) -> bool {
        !matches!(self, Value::None)
    }
}

fn fish(bin: &str, spec: &Spec) -> String {
    let f = format!("__{}", ident(bin));
    let all = spec.all();
    let subs: String = all
        .iter()
        .filter(|cmd| !cmd.subcommands.is_empty())
        .map(|cmd| {
            let names: Vec<&str> = cmd.subcommands.iter().map(|s| s.name()).collect();
            format!(
                "        case {}\n            printf '%s\\n' {}\n",
                fish_quoted(&cmd.path),
                names.join(" ")
            )
        })
        .collect();
    let mut ends: Vec<String> = all
        .iter()
        .flat_map(|cmd| cmd.options.iter().filter(|o| o.ends).flat_map(Opt::forms))
        .collect();
    ends.sort();
    ends.dedup();
    let mut script = format!(
        r#"# fish completion for {bin}

# The subcommands of the command at path $argv[1].
function {f}_subs
    switch $argv[1]
{subs}    end
end

# Whether the command line is at the command with path $argv[1], and nothing ended it. With
# $argv[2], also whether exactly that many words follow the command.
function {f}_at
    set -l words (commandline -opc)[2..-1]
    for word in $words
        contains -- $word {ends}; and return 1
    end
    set -l path ''
    while set -q words[1]; and contains -- $words[1] ({f}_subs $path)
        if test -z "$path"
            set path $words[1]
        else
            set path $path/$words[1]
        end
        set -e words[1]
    end
    test "$path" = "$argv[1]"; or return 1
    set -q argv[2]; or return 0
    test (count $words) -eq $argv[2]
end

# Whether one of the option forms in $argv, such as -d or --dir, was given. With -l first, the last
# word is left out: it is the option whose value is being completed.
function {f}_given
    set -l words (commandline -opc)[2..-1]
    if test "$argv[1]" = -l
        set -e argv[1]
        set -e words[-1]
    end
    for word in $words
        for form in $argv
            test "$word" = $form; or string match -q -- "$form=*" $word; and return 0
        end
    end
    return 1
end

complete -c {bin} -f
"#,
        ends = ends.join(" ")
    );
    for cmd in all {
        let at = format!("{f}_at {}", fish_quoted(&cmd.path));
        for sub in &cmd.subcommands {
            writeln!(
                script,
                "complete -c {bin} -n {} -a {} -d {}",
                fish_quoted(&format!("{at} 0")),
                fish_quoted(sub.name()),
                fish_quoted(&sub.about)
            )
            .unwrap();
        }
        for opt in &cmd.options {
            // The option itself may be the last word, when its value is being completed.
            let last = if opt.value.takes() { " -l" } else { "" };
            let mut condition = format!("{at}; and not {f}_given{last} {}", opt.forms().join(" "));
            for &c in &opt.conflicts {
                write!(
                    condition,
                    "; and not {f}_given {}",
                    cmd.options[c].forms().join(" ")
                )
                .unwrap();
            }
            let mut line = format!("complete -c {bin} -n {}", fish_quoted(&condition));
            for s in &opt.shorts {
                write!(line, " -s {s}").unwrap();
            }
            for l in &opt.longs {
                write!(line, " -l {l}").unwrap();
            }
            match &opt.value {
                Value::None => {}
                Value::Any => line.push_str(" -x"),
                Value::Directory => {
                    line.push_str(" -x -a '(__fish_complete_directories (commandline -ct))'")
                }
                Value::Choices(choices) => {
                    write!(line, " -x -a {}", fish_quoted(&choices.join(" "))).unwrap()
                }
            }
            writeln!(script, "{line} -d {}", fish_quoted(&opt.help)).unwrap();
        }
        for (index, value) in cmd.positionals.iter().enumerate() {
            let condition = fish_quoted(&format!("{at} {index}"));
            match value {
                Value::Choices(choices) => {
                    writeln!(script, "complete -c {bin} -n {condition} -a {}", fish_quoted(&choices.join(" "))).unwrap()
                }
                Value::Directory => writeln!(
                    script,
                    "complete -c {bin} -n {condition} -a '(__fish_complete_directories (commandline -ct))'"
                )
                .unwrap(),
                Value::None | Value::Any => {}
            }
        }
    }
    script
}

/// `text` in single quotes, for fish.
fn fish_quoted(text: &str) -> String {
    format!("'{}'", text.replace('\\', r"\\").replace('\'', r"\'"))
}
