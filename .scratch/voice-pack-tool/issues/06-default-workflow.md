# 06: Default workflow

**What to build:** Running `fix-whxm4-voice` with no subcommand sets up the workspace (with ticket 01's prompt rules), downloads the originals if they're missing, and finishes by explaining where to put changes (same names as in `original/`, any format, empty file means silence) and which command uploads them. Re-running it skips the download when originals are present. The offer to upload comes in ticket 10.

**Blocked by:** 02 (Download originals)

**Status:** ready-for-agent

- [ ] A first run in an empty directory ends with originals downloaded and next-step instructions printed
- [ ] A re-run doesn't download again
- [ ] The global flags `-d`, `-f` and `--lang` apply to the default command
