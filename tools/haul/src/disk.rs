//! Free disk, via `df`, with the parser tested against canned output.
//!
//! Rust's standard library cannot ask how much space is left on a filesystem,
//! and this crate takes no dependencies. `df -Pk` is POSIX-specified output;
//! the risk is not the command, it is a parser that silently returns 0 or
//! `None` on a line it did not expect — which would make the disk alarm read
//! healthy forever. So the parser is a pure function with tests, including one
//! for output it must refuse.

pub fn parse_df(out: &str) -> Result<i64, String> {
    let mut lines = out.lines().filter(|l| !l.trim().is_empty());
    let header = lines.next().ok_or("df printed nothing")?;
    if !header.to_ascii_lowercase().contains("available") {
        return Err(format!("df header not recognised: {header:?}"));
    }
    // POSIX `-P` guarantees one line per filesystem, fields:
    // Filesystem 1024-blocks Used Available Capacity Mounted-on
    let line = lines.next().ok_or("df printed a header and no filesystem")?;
    let fields: Vec<&str> = line.split_whitespace().collect();
    if fields.len() < 6 {
        return Err(format!("df row has {} fields, expected 6: {line:?}", fields.len()));
    }
    let kb: i64 = fields[3]
        .parse()
        .map_err(|_| format!("df available field {:?} is not a number", fields[3]))?;
    Ok(kb / 1024)
}

pub fn free_mb(path: &std::path::Path) -> Result<i64, String> {
    let out = std::process::Command::new("df")
        .args(["-Pk", &path.to_string_lossy()])
        .output()
        .map_err(|e| format!("spawn df: {e}"))?;
    if !out.status.success() {
        return Err(format!("df failed: {}", String::from_utf8_lossy(&out.stderr).trim()));
    }
    parse_df(&String::from_utf8_lossy(&out.stdout))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_real_df_output() {
        let out = "Filesystem     1024-blocks      Used Available Capacity Mounted on\n\
                   /dev/nvme0n1p1  1922000000 598000000 1226000000      33% /\n";
        assert_eq!(parse_df(out).unwrap(), 1_226_000_000 / 1024);
    }

    #[test]
    fn refuses_output_it_does_not_understand_instead_of_returning_zero() {
        // The failure that matters: a parser that answers 0 makes the disk
        // alarm scream forever; one that answers a huge number makes it never
        // fire. Both are worse than an error.
        assert!(parse_df("").is_err());
        assert!(parse_df("df: /nope: No such file or directory\n").is_err());
        assert!(parse_df("Filesystem Available\nonly two fields\n").is_err());
        assert!(parse_df("Filesystem 1024-blocks Used Available Capacity Mounted on\n\
                          /dev/x 100 100 not-a-number 100% /\n")
            .is_err());
    }
}
