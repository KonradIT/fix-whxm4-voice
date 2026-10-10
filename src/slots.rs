//! The slot catalogue: which prompt each slot of a voice pack belongs to.

/// Number of slots in a WH-1000XM4 voice pack.
pub const SLOT_COUNT: usize = 54;

/// The prompt name of every slot, as used in file names. Speech slots are named after what the
/// English pack says; tones, whose events are not known yet, are `unknown`.
const PROMPTS: [&str; SLOT_COUNT] = [
    "power-on",                                   // 0
    "power-off",                                  // 1
    "bluetooth-pairing",                          // 2
    "bluetooth-connected",                        // 3
    "bluetooth-disconnected",                     // 4
    "recharge-power-off",                         // 5
    "noise-canceling",                            // 6
    "ambient-sound-off",                          // 7
    "ambient-sound",                              // 8
    "google-assistant-unavailable-during-update", // 9
    "unknown",                                    // 10
    "unknown",                                    // 11
    "unknown",                                    // 12
    "unknown",                                    // 13
    "unknown",                                    // 14
    "battery-full",                               // 15
    "battery-70",                                 // 16
    "battery-50",                                 // 17
    "battery-20",                                 // 18
    "unknown",                                    // 19
    "unknown",                                    // 20
    "google-assistant-not-connected",             // 21
    "unknown",                                    // 22
    "unknown",                                    // 23
    "alexa-not-connected",                        // 24
    "unknown",                                    // 25
    "unknown",                                    // 26
    "xiaowei-not-connected",                      // 27
    "battery-90",                                 // 28
    "battery-80",                                 // 29
    "battery-60",                                 // 30
    "battery-40",                                 // 31
    "battery-30",                                 // 32
    "battery-10",                                 // 33
    "low-battery",                                // 34
    "optimizer-start",                            // 35
    "optimizer-finished",                         // 36
    "unknown",                                    // 37
    "unknown",                                    // 38
    "unknown",                                    // 39
    "unknown",                                    // 40
    "unknown",                                    // 41
    "unknown",                                    // 42
    "speak-to-chat-on",                           // 43
    "speak-to-chat-off",                          // 44
    "unknown",                                    // 45
    "unknown",                                    // 46
    "device-1-connected",                         // 47
    "device-1-disconnected",                      // 48
    "device-2-connected",                         // 49
    "device-2-disconnected",                      // 50
    "device-1-replaced",                          // 51
    "device-2-replaced",                          // 52
    "second-device-connected",                    // 53
];

/// The prompt name of `slot`.
pub fn prompt(slot: usize) -> &'static str {
    PROMPTS[slot]
}

/// The file name of the clip in `slot`, such as `07-ambient-sound-off.mp3`.
pub fn clip_file_name(slot: usize) -> String {
    format!("{slot:02}-{}.mp3", prompt(slot))
}
