# 01: Workspace and flake skeleton

**What to build:** A runnable `fix-whxm4-voice` binary from the flake that sets up a **workspace**. `nix run .# -- -d foo` creates `foo/` (if missing) with `original/`, `changes/` and `build/`. When those folders are being created in a directory that already contains other things, the tool explains that it should run in its own directory and asks whether to continue. Hidden entries don't count. `-f` skips the question. Once the folders exist, it never asks again. Subcommands may be stubs that report "not implemented yet".

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

- [ ] The flake provides a package (for `nix run .#`), wrapped so ffmpeg is on its PATH, and a devShell with the Rust toolchain, ffmpeg and jadx
- [ ] The workspace defaults to the current directory; `-d <dir>` selects another and creates it if missing
- [ ] A directory with non-hidden entries other than `original/`, `changes/`, `build/` triggers the explain-and-ask prompt, but only when one of those folders is about to be created
- [ ] Hidden entries (`.envrc`, `.direnv/`, `.git/`, …) never trigger the prompt
- [ ] `-f` skips the prompt; declining exits without creating anything
- [ ] The CLI defines the subcommands `download`, `build`, `upload`, `device-info` and the default command (stubs are fine)
