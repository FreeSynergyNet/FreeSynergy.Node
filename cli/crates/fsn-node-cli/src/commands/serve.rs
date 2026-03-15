// `fsn serve` — web management UI (now part of FreeSynergy.Desktop).

use std::path::Path;
use anyhow::Result;

pub async fn run(_root: &Path, _project: Option<&Path>, _bind: &str, _port: u16) -> Result<()> {
    eprintln!("The web UI is now part of FreeSynergy.Desktop.");
    eprintln!("Run `fsd` to open the desktop in web mode.");
    Ok(())
}
