use std::{
    collections::{ HashMap, HashSet },
    fs::{ self, File, FileType, OpenOptions },
    io::{ BufReader, Read, Write },
    path::PathBuf,
    str::FromStr,
    time::Duration,
};

use clap::{ builder::Str, Parser };
use colored::Colorize;
use indicatif::{ MultiProgress, ProgressBar, ProgressState, ProgressStyle };
use rayon::iter::{ IntoParallelRefIterator, ParallelIterator };
use ring::digest::{ Context, SHA256 };

#[derive(Debug, Parser)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    path: Option<PathBuf>,

    #[arg(short, default_value_t = false)]
    compare: bool,

    #[arg(short, long)]
    files_to_compare: Option<Vec<PathBuf>>,

    #[arg(short, long, default_value_t = false)]
    disable_progress_bar: bool,

    #[arg(short, long, default_value_t = 1024 * 100)]
    buffer_size: usize,
}

enum CompareResult {
    Same,
    Different,
    Missing(u8),
}

fn read_to_hashmap<R: Read>(
    source: &mut R
) -> Result<HashMap<String, String>, Box<dyn std::error::Error>> {
    let mut res: HashMap<String, String> = HashMap::new();
    let mut buf = String::new();
    source.read_to_string(&mut buf)?;
    buf.split("\n")
        .into_iter()
        .for_each(|line| {
            let splited = line.split(">").collect::<Vec<&str>>();
            if splited.len() != 2 {
                return;
            }
            res.insert(splited[0].to_string(), splited[1].to_string());
        });
    return Ok(res);
}

fn compare<R: Read>(
    file1: &mut R,
    file2: &mut R
) -> Result<HashMap<String, CompareResult>, Box<dyn std::error::Error>> {
    let mut hashmap1 = read_to_hashmap(file1)?;
    let mut hashmap2 = read_to_hashmap(file2)?;

    let mut result: HashMap<String, CompareResult> = HashMap::new();

    let comp_func = |key: &String, value: &String, hashmap: &mut HashMap<String, String>| {
        if let Some(v) = hashmap.get(key) {
            if *v == *value { CompareResult::Same } else { CompareResult::Different }
        } else {
            CompareResult::Missing(0)
        }
    };

    for (key, value) in hashmap1.iter() {
        let res = comp_func(key, value, &mut hashmap2);
        if let CompareResult::Missing(_) = res {
            result.insert(key.clone(), CompareResult::Missing(1));
            continue;
        }
        result.insert(key.clone(), res);
        hashmap2.remove(key);
    }
    for (key, value) in hashmap2.iter() {
        let res = comp_func(key, value, &mut hashmap1);
        if let CompareResult::Missing(_) = res {
            result.insert(key.clone(), CompareResult::Missing(0));
            continue;
        }
        result.insert(key.clone(), res);
    }

    Ok(result)
}

fn collect_checksums() {
    //TODO: move
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    if args.compare {
        let files = args.files_to_compare.ok_or("missing paths")?;
        if files.len() != 2 {
            return Err(format!("Wrong number of paths {} insted of 2", files.len()).into());
        }

        let file1 = File::open(&files[0])?;
        let file2 = File::open(&files[1])?;

        let mut reader1 = BufReader::new(file1);
        let mut reader2 = BufReader::new(file2);
        let res = compare(&mut reader1, &mut reader2)?;

        let mut missing_count = 0;
        let mut same_count = 0;
        let mut diffrent_count = 0;
        for (key, value) in res.into_iter() {
            match value {
                CompareResult::Same => {
                    same_count += 1;
                }
                CompareResult::Different => {
                    println!("{}", format!("file at path {} is diffrent", key).yellow());
                    diffrent_count += 1;
                }
                CompareResult::Missing(file_index) => {
                    let file_name = files[file_index as usize].file_name();

                    println!(
                        "{}",
                        format!(
                            "the file at path {} is missing in {}",
                            key,
                            file_name
                                .ok_or(
                                    format!(
                                        "Error while getting file name for {}",
                                        files[file_index as usize].as_os_str().to_str().unwrap()
                                    )
                                )?
                                .to_str()
                                .unwrap()
                        ).red()
                    );
                    missing_count += 1;
                }
            }
        }
        println!(
            "same: {}\ndiffrent: {}\nmissing: {}\n",
            same_count,
            diffrent_count,
            missing_count
        );
        return Ok(());
    }

    if args.path.is_none() {
        println!("Path is None");
        return Ok(());
    }

    // let path = PathBuf::from_str(&args.path.unwrap())?;

    let origin_path = args.path.unwrap();

    let mut dirs_to_check = vec![origin_path.clone()];

    let mut out_file: File = OpenOptions::new().create_new(true).write(true).open("./out.txt")?;

    let progress_style = ProgressStyle::with_template(
        "[{elapsed_precise}] {bar:60} {pos:>7}/{len:7} {msg}"
    ).unwrap();
    let total_progress = ProgressBar::new(1).with_style(progress_style);
    total_progress.enable_steady_tick(Duration::from_secs(1));

    let progress_bars = MultiProgress::new();
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
                    "{bar:60.cyan/white} {bytes}/{total_bytes} ({eta})"
                )
                    .unwrap()
                    .progress_chars("#>-");
                let local_progress = progress_bars
                    .add(ProgressBar::new(size))
                    .with_style(progress_style)
                    .with_message(file_name);

                let mut reader = BufReader::new(file);

                let mut context = Context::new(&SHA256);

                let mut buffer = [0; 1024 * 100];
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
            let path = path.to_str().unwrap();
            if let Err(err) = checkum_res {
                println!("Error while geting checksum of {} {}", path, err);
                continue;
            }
            out_file.write(
                format!(
                    "{}>{}\n",
                    path.replace(origin_path.to_str().unwrap(), ""),
                    checkum_res.as_ref().unwrap()
                ).as_bytes()
            )?;
        }
        progress_bars.remove(&dir_progress);
        total_progress.inc(1);
    }
    total_progress.set_message("finished!");
    total_progress.finish();
    Ok(())
}
