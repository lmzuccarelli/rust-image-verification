// module schema

use clap::Parser;

/// rust-container-tool cli struct
#[derive(Parser, Debug)]
#[command(name = "rust-image-verification")]
#[command(author = "Luigi Mario Zuccarelli <luzuccar@redhat.com>")]
#[command(version = "0.2.0")]
#[command(about = "Used to verify blob integrity (using manifest to check file size & blob contents hashing sha256 with digest in manifest)", long_about = None)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    /// base directory
    #[arg(
        short,
        long,
        value_name = "base-dir",
        required = true,
        default_value = ""
    )]
    pub base_dir: String,

    /// release directory to check
    #[arg(short, long, value_name = "release-dir")]
    pub release_dir: Option<String>,

    /// operator directory to check
    #[arg(short, long, value_name = "operators-dir")]
    pub operators_dir: Option<String>,

    /// set the loglevel. Valid arguments are info, debug, trace
    #[arg(value_enum, long, value_name = "loglevel", default_value = "info")]
    pub loglevel: Option<String>,
}
