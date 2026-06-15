use std::path::PathBuf;

pub type LoginStatusLine = fn(&[(String, String)]) -> String;

#[derive(Debug, Clone, Default)]
pub struct PollOptions {
    pub keep_browser_open: bool,
    pub user_data_dir: Option<PathBuf>,
    pub chrome_executable: Option<PathBuf>,
    pub status_line: Option<LoginStatusLine>,
}
