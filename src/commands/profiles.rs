use std::path::Path;

use anyhow::Result;

use crate::output::{self, OutputFormat};
use crate::profile;

pub fn run(brave_dir: &Path, format: OutputFormat) -> Result<()> {
    let profiles = profile::list_profiles(brave_dir)?;
    output::print_profiles(&profiles, format);
    Ok(())
}
