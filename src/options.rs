use std::path::PathBuf;

#[derive(Clone, PartialEq)]
pub struct Options {
    /// Model files (.bin) are picked from this directory; VAD models have "silero" in
    /// the name.
    pub models_dir: PathBuf,
    pub model: Option<PathBuf>,
    pub vad: Option<PathBuf>,
    pub language: &'static str,
    /// Language codes offered in the selection, with "auto" for whisper's detection.
    pub languages: &'static [(&'static str, &'static str)],
    pub pause_ms: u32,
    pub pause_min_ms: u32,
    pub pause_max_ms: u32,
    pub diag_window: usize,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            models_dir: PathBuf::from("models"),
            model: None,
            vad: None,
            language: "en",
            languages: &[
                ("en", "English"),
                ("auto", "Auto-detect"),
                ("lt", "Lithuanian"),
                ("de", "German"),
                ("fr", "French"),
                ("es", "Spanish"),
                ("pl", "Polish"),
                ("ru", "Russian"),
                ("uk", "Ukrainian"),
            ],
            pause_ms: 300,
            pause_min_ms: 100,
            pause_max_ms: 1000,
            diag_window: 20,
        }
    }
}
