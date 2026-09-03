use std::{env, fs};

#[derive(Clone)]
struct Instruction {
    address: u64,
    code: String,
    shape: String,
}

fn replace_hex_numbers(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = String::with_capacity(text.len());
    let mut index = 0;
    while index < bytes.len() {
        if index + 2 < bytes.len() && bytes[index] == b'0' && bytes[index + 1] == b'x' {
            let mut end = index + 2;
            while end < bytes.len() && bytes[end].is_ascii_hexdigit() {
                end += 1;
            }
            if end > index + 2 {
                out.push_str("NUM");
                index = end;
                continue;
            }
        }
        out.push(bytes[index] as char);
        index += 1;
    }
    out
}

fn normalize_rip(mut text: String) -> String {
    loop {
        let Some(position) = text.find("rip+0x").or_else(|| text.find("rip-0x")) else {
            break;
        };
        let start = position + 3;
        let mut end = start + 3;
        while end < text.len() && text.as_bytes()[end].is_ascii_hexdigit() {
            end += 1;
        }
        text.replace_range(start..end, "+DISP");
    }
    text
}

fn parse(path: &str) -> Vec<Instruction> {
    let text = fs::read_to_string(path).expect("read disassembly");
    let mut instructions = Vec::new();
    for line in text.lines() {
        let Some((left, right)) = line.split_once(":\t") else {
            continue;
        };
        let address_text = left.trim();
        if address_text.is_empty() || !address_text.chars().all(|ch| ch.is_ascii_hexdigit()) {
            continue;
        }
        let mnemonic = right.split_whitespace().next().unwrap_or("");
        if matches!(mnemonic, "int3" | "(bad)" | "nop") || mnemonic.starts_with("nopw") {
            continue;
        }
        let mut code = normalize_rip(right.split('#').next().unwrap_or("").trim().to_owned());
        if mnemonic == "call" || mnemonic == "jmp" || mnemonic.starts_with('j') {
            if let Some(space) = code.find(char::is_whitespace) {
                let operands = code[space..].trim();
                let first = operands
                    .split_whitespace()
                    .next()
                    .unwrap_or("")
                    .trim_end_matches(',');
                let numeric = first.trim_start_matches("0x");
                if !numeric.is_empty() && numeric.chars().all(|ch| ch.is_ascii_hexdigit()) {
                    code = format!("{mnemonic} ADDR{}", &operands[first.len()..]);
                }
            }
        }
        instructions.push(Instruction {
            address: u64::from_str_radix(address_text, 16).expect("instruction address"),
            shape: replace_hex_numbers(&code),
            code,
        });
    }
    instructions
}

fn align(old: &[Instruction], new: &[Instruction]) -> Vec<(Option<usize>, Option<usize>)> {
    let rows = old.len();
    let columns = new.len();
    let width = columns + 1;
    let mut distance = vec![0_u16; (rows + 1) * width];
    for row in 0..=rows {
        distance[row * width] = row as u16;
    }
    for column in 0..=columns {
        distance[column] = column as u16;
    }
    for row in 1..=rows {
        for column in 1..=columns {
            let substitution_cost = u16::from(old[row - 1].shape != new[column - 1].shape);
            distance[row * width + column] = distance[(row - 1) * width + column - 1]
                .saturating_add(substitution_cost)
                .min(distance[(row - 1) * width + column].saturating_add(1))
                .min(distance[row * width + column - 1].saturating_add(1));
        }
    }
    let (mut row, mut column) = (rows, columns);
    let mut pairs = Vec::new();
    while row > 0 || column > 0 {
        if row > 0 && column > 0 {
            let cost = u16::from(old[row - 1].shape != new[column - 1].shape);
            if distance[row * width + column]
                == distance[(row - 1) * width + column - 1].saturating_add(cost)
            {
                pairs.push((Some(row - 1), Some(column - 1)));
                row -= 1;
                column -= 1;
                continue;
            }
        }
        if row > 0
            && distance[row * width + column]
                == distance[(row - 1) * width + column].saturating_add(1)
        {
            pairs.push((Some(row - 1), None));
            row -= 1;
        } else {
            pairs.push((None, Some(column - 1)));
            column -= 1;
        }
    }
    pairs.reverse();
    pairs
}

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.len() != 2 {
        eprintln!("usage: alignmap OLD.s NEW.s");
        std::process::exit(2);
    }
    let old = parse(&args[0]);
    let new = parse(&args[1]);
    println!("old_site\tnew_site\tshape_equal\tnorm_equal\told_code\tnew_code");
    for (old_index, new_index) in align(&old, &new) {
        let (Some(old_index), Some(new_index)) = (old_index, new_index) else {
            continue;
        };
        println!(
            "0x{:x}\t0x{:x}\t{}\t{}\t{}\t{}",
            old[old_index].address,
            new[new_index].address,
            old[old_index].shape == new[new_index].shape,
            old[old_index].code == new[new_index].code,
            old[old_index].code,
            new[new_index].code,
        );
    }
}
