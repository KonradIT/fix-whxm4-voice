# 05: Size limit report

**What to build:** If the clips together are larger than the stock clips of the base language,
`build` fails with a report: each clip's size, the total, the limit and where it comes from, and a
ready-to-run ffmpeg command for making the oversized changes smaller. The user doesn't need to know
ffmpeg.

**Blocked by:** 04 (Changes override originals)

**Status:** ready-for-agent

- [ ] The limit is the stock clip total of the base language and lives in one place, so it can later
      be replaced by a hardcoded constant
- [ ] The report lists every clip with its size (changed ones marked), the total, and the limit
- [ ] The suggested ffmpeg command(s) target the user's oversized change files and keep the stock format
- [ ] A pack within the limit builds normally with no report
