use std::{
    fs::{ self, File, OpenOptions },
    io::{ stdout, BufReader, Read, Write },
    time::Duration,
};

use indicatif::{ MultiProgress, ProgressBar, ProgressStyle };
use rayon::iter::{ IntoParallelRefIterator, ParallelIterator };
use ring::digest::{ Context, SHA256 };
use anyhow::Result;

use crate::Args;

pub fn run(args: Args) -> Result<()> {
    if args.paths.is_none() {
        println!("Path is None");
        return Ok(());
    }

    // let pool = rayon::ThreadPoolBuilder::new().num_threads(10)

    crossterm::execute!(stdout(), crossterm::terminal::Clear(crossterm::terminal::ClearType::All))?;
    let paths = args.paths.unwrap();

    let origin_path = paths[0].clone();

    let mut dirs_to_check = vec![origin_path.clone()];

    let out_file = OpenOptions::new().create_new(true).write(true).open(&args.out_path);
    if let Err(err) = out_file {
        println!("Failed to open {} {}", args.out_path, err);
        return Ok(());
    }
    let mut out_file = out_file.unwrap();

    let progress_style = ProgressStyle::with_template(
        "[{elapsed_precise}] {bar:60} {pos:>7}/{len:7} {msg}"
    ).unwrap();
    let total_progress = ProgressBar::new(1).with_style(progress_style);
    total_progress.enable_steady_tick(Duration::from_secs(1));

    let progress_bars = MultiProgress::new();
    progress_bars.set_move_cursor(false);
    let total_progress = progress_bars.add(total_progress);

    while let Some(dir) = dirs_to_check.pop() {
        let mut file_path = Vec::new();

        let dir_name = if let Some(dir_name) = &dir.file_name() {
            dir_name.to_str().unwrap().to_string()
        } else {
            dir.to_str().unwrap().to_string()
        };

        total_progress.set_message(format!("current dir: {}", dir_name));

        for entry in fs::read_dir(dir)? {
            if let Err(e) = entry {
                panic!("error while reading file: {}", e);
            }
            let entry = entry.unwrap();
            let metadata = entry.metadata()?;

            if metadata.is_dir() {
                // metadata.len()
                dirs_to_check.push(entry.path());

                total_progress.set_length(total_progress.length().unwrap() + 1);
                continue;
            }
            file_path.push(entry.path());
        }

        let dir_progress = progress_bars.add(
            ProgressBar::new(file_path.len() as u64).with_style(
                ProgressStyle::with_template("{bar:60.green/white} {pos:>7}/{len:7}").unwrap()
            )
        );
        dir_progress.enable_steady_tick(Duration::from_secs(3));

        let res: Vec<Result<String, String>> = file_path
            .par_iter()
            .map(|path| {
                let file = File::open(path);
                if let Err(err) = file {
                    return Err(err.to_string());
                }
                let file = file.unwrap();
                let metadata = file.metadata().unwrap();

                let size = metadata.len();
                let file_name = path.file_name().unwrap().to_str().unwrap().to_string();
                let progress_style = ProgressStyle::with_template(
                    "{bar:60.cyan/white} {bytes}/{total_bytes} {msg} ({eta})"
                )
                    .unwrap()
                    .progress_chars("#>-");
                let local_progress = progress_bars
                    .add(ProgressBar::new(size))
                    .with_style(progress_style)
                    .with_message(file_name);

                let mut reader = BufReader::new(file);

                let mut context = Context::new(&SHA256);

                let mut buffer = vec![0; args.buffer_size];
                loop {
                    let read_res = reader.read(&mut buffer);
                    match read_res {
                        Ok(count) => {
                            if count == 0 {
                                break;
                            }
                            context.update(&buffer[..count]);
                            local_progress.inc(count as u64);
                        }
                        Err(e) => {
                            return Err(e.to_string());
                        }
                    }
                }
                let digest = context.finish();
                local_progress.finish_and_clear();
                progress_bars.remove(&local_progress);

                dir_progress.inc(1);
                Ok(data_encoding::HEXLOWER.encode(digest.as_ref()))
            })
            .collect();

        for (index, path) in file_path.iter().enumerate() {
            let checkum_res = &res[index];
            let absoulte_path = path.canonicalize()?;
            let path = absoulte_path.to_str().unwrap();
            if let Err(err) = checkum_res {
                println!("Error while geting checksum of {} {}", path, err);
                continue;
            }

            out_file.write(
                format!(
                    "{}>{}\n",
                    path.replace(origin_path.canonicalize()?.to_str().unwrap(), ""),
                    checkum_res.as_ref().unwrap()
                ).as_bytes()
            )?;
        }
        progress_bars.remove(&dir_progress);
        progress_bars.clear()?;
        total_progress.inc(1);
    }
    total_progress.set_message("finished!");
    total_progress.finish();
    Ok(())
}
