use std::time::Duration;

use indicatif::{ProgressBar, ProgressStyle};

const HTTP_CONNECT_TIMEOUT: Duration = Duration::from_secs(30);
const HTTP_READ_TIMEOUT: Duration = Duration::from_secs(30);
const HTTP_DOWNLOAD_READ_TIMEOUT: Duration = Duration::from_secs(600);

fn agent_builder() -> ureq::AgentBuilder {
    let user_agent = format!(
        "packtrans/glossary/{} (https://github.com/packtrans/glossary)",
        env!("CARGO_PKG_VERSION")
    );
    ureq::AgentBuilder::new().user_agent(&user_agent)
}

/// HTTP client for API metadata requests (short responses).
pub fn http_client() -> ureq::Agent {
    agent_builder()
        .timeout_connect(HTTP_CONNECT_TIMEOUT)
        .timeout_read(HTTP_READ_TIMEOUT)
        .build()
}

/// HTTP client for large file downloads (mod jars, Minecraft assets).
pub fn http_download_client() -> ureq::Agent {
    agent_builder()
        .timeout_connect(HTTP_CONNECT_TIMEOUT)
        .timeout_read(HTTP_DOWNLOAD_READ_TIMEOUT)
        .build()
}

pub fn progress_bar(len: u64, message: &'static str) -> ProgressBar {
    let pb = ProgressBar::new(len);
    pb.set_style(
        ProgressStyle::default_bar()
            .template(
                "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({msg})",
            )
            .unwrap()
            .progress_chars("#>-"),
    );
    pb.set_message(message);
    pb
}
