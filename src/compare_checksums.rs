use anyhow::Result;
use std::{ collections::HashMap, fs::File, io::{ BufReader, Read } };
use colored::Colorize;

use crate::Args;
enum CompareResult {
    Same,
    Different,
    Missing(u8),
}

fn read_to_hashmap<R: Read>(source: &mut R) -> Result<HashMap<String, String>> {
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
    Ok(res)
}

fn compare<R: Read>(file1: &mut R, file2: &mut R) -> Result<HashMap<String, CompareResult>> {
    let hashmap1 = read_to_hashmap(file1)?;
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
    for key in hashmap2.keys() {
        result.insert(key.clone(), CompareResult::Missing(0));
    }

    Ok(result)
}

pub fn run(args: Args) -> Result<()> {
    let files = args.paths.ok_or(anyhow::format_err!("missing paths"))?;
    if files.len() != 2 {
        return Err(anyhow::format_err!("Wrong number of paths {} insted of 2", files.len()));
    }

    let file1 = File::open(&files[0]).map_err(|err| {
        anyhow::anyhow!(
            "{} while trying to open {:?}",
            err,
            files[0].file_name().ok_or(format!("file at index {}", 0))
        )
    })?;
    let file2 = File::open(&files[1]).map_err(|err| {
        anyhow::anyhow!(
            "{} while trying to open {:?}",
            err,
            files[0].file_name().ok_or(format!("file at index {}", 1))
        )
    })?;

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
                println!("{}", format!("{} is diffrent", key).yellow());
                diffrent_count += 1;
            }
            CompareResult::Missing(file_index) => {
                let file_name = files[file_index as usize].file_name();
                println!(
                    "{}",
                    format!(
                        "{} [missing in {}]",
                        key,
                        file_name
                            .ok_or(
                                anyhow::format_err!(
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
    println!("same: {}\ndiffrent: {}\nmissing: {}\n", same_count, diffrent_count, missing_count);
    Ok(())
}
