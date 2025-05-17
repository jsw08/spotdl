use clap::Parser;

/// Simple spotify ripper, for (relatively) high quality offline audio files!.
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Args {
    #[clap(flatten)]
    pub source: CliSource,

    /// Path to folder where the songs gets downloaded. Will otherwise create a new folder in the current directory.
    #[arg(long)]
    pub path: Option<std::path::PathBuf>,

    /// Time to wait (in milliseconds) between downloading songs. It triggers every 10 songs. Prevents spotify from banning your account.
    #[clap(default_value_t = 5000)]
    pub timeout: u64,
}

#[derive(Debug, clap::Args)]
#[group(required = true, multiple = false)]
pub struct CliSource {
    /// From a spotify uri. Must be in the following format: "spotify:playlist:..." or ""spotify:track:...""
    #[arg(short, long)]
    pub uri: Option<String>,
}
