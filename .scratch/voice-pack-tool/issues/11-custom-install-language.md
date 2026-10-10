# 11: "Custom" install language

**What to build:** Install the pack under a dedicated "custom" **install language**, so the app shows it as custom and the "same language is ignored" problem goes away. Only do this if ticket 07's findings show that an unknown language code is safe on the device side. Otherwise close the ticket with the reasons written down.

**Blocked by:** 10 (Real upload)

**Status:** ready-for-agent

- [ ] Ticket 07's findings on language codes are reviewed and the safety argument written down before any attempt
- [ ] If safe: `upload` uses the custom code by default and falls back to a different real language on request
- [ ] If safe: verified by hand that the headphones play the pack and what the app then shows
- [ ] If not safe or not possible: the ticket documents why, and the real-language behaviour from ticket 10 stays
