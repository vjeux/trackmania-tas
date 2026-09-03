use std::{collections::HashMap, env, fs};

#[derive(Clone)]
struct Row {
    old_site: u64,
    new_site: u64,
    context: String,
    old_offset: i64,
    new_offset: i64,
    old_code: String,
    new_code: String,
}

fn number(text: &str) -> i64 {
    let stripped = text.trim().trim_start_matches("0x");
    u64::from_str_radix(stripped, 16).expect("hex number") as i64
}

fn rows(path: &str) -> Vec<Row> {
    fs::read_to_string(path)
        .expect("read field map")
        .lines()
        .skip(1)
        .filter_map(|line| {
            let fields: Vec<&str> = line.split('\t').collect();
            if fields.len() < 7 {
                return None;
            }
            Some(Row {
                old_site: number(fields[0]) as u64,
                new_site: number(fields[1]) as u64,
                context: fields[2].to_owned(),
                old_offset: number(fields[3]),
                new_offset: number(fields[4]),
                old_code: fields[5].to_owned(),
                new_code: fields[6].to_owned(),
            })
        })
        .collect()
}

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.len() != 2 {
        eprintln!("usage: compose_fields OLD_TO_MIDDLE.tsv MIDDLE_TO_NEW.tsv");
        std::process::exit(2);
    }
    let first = rows(&args[0]);
    let second = rows(&args[1]);
    let mut by_site = HashMap::<u64, Vec<Row>>::new();
    for row in second {
        by_site.entry(row.old_site).or_default().push(row);
    }
    println!("old_site\tmiddle_site\tnew_site\tcontext\told_off\tmiddle_off\tnew_off\told_code\tnew_code");
    let mut count = 0;
    for left in first {
        let Some(candidates) = by_site.get(&left.new_site) else {
            continue;
        };
        for right in candidates {
            if left.context != right.context || left.new_offset != right.old_offset {
                continue;
            }
            println!(
                "0x{:x}\t0x{:x}\t0x{:x}\t{}\t{:#x}\t{:#x}\t{:#x}\t{}\t{}",
                left.old_site,
                left.new_site,
                right.new_site,
                left.context,
                left.old_offset,
                left.new_offset,
                right.new_offset,
                left.old_code,
                right.new_code,
            );
            count += 1;
        }
    }
    eprintln!("composed_field_rows={count}");
}
