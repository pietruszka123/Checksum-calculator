use std::path::PathBuf;
use clap::Parser;
use anyhow::Result;

mod genreate_checksums;
mod compare_checksums;

#[derive(Debug, Parser)]
#[command(version, about, long_about = None)]
struct Args {
    /// Paths to the files to be processed.
    #[arg(short, long)]
    paths: Option<Vec<PathBuf>>,

    /// Compare two output files.
    #[arg(short, default_value_t = false)]
    compare: bool,

    /// The buffer size for reading files.
    #[arg(short, long, default_value_t = 1024 * 100)]
    buffer_size: usize,

    /// The output file path.
    #[arg(short, long, default_value = "./out.txt")]
    out_path: String,
}

fn main() -> Result<()> {
    let args = Args::parse();

    if args.compare {
        compare_checksums::run(args)
    } else {
        genreate_checksums::run(args)
    }
}
