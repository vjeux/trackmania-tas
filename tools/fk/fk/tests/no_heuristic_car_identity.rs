use std::path::{Path, PathBuf};

fn source_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            if name != "target" && name != ".git" && name != "evidence" && name != "testdata" {
                source_files(&path, out);
            }
        } else if matches!(
            path.extension().and_then(|s| s.to_str()),
            Some("rs" | "toml")
        ) {
            out.push(path);
        }
    }
}

#[test]
fn production_has_no_heuristic_car_identity_escape_hatch() {
    let tools = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let forbidden = [
        ["locate", "_v2"].concat(),
        ["locate", "_pos2"].concat(),
        ["locate", "_candidates"].concat(),
        ["locate", "_positions_loose"].concat(),
        ["locate", "_blind"].concat(),
        ["FK", "_STATE_OFF"].concat(),
        ["state", "_off"].concat(),
    ];
    let mut files = Vec::new();
    source_files(&tools, &mut files);
    let mut hits = Vec::new();
    for path in files {
        let text = std::fs::read_to_string(&path).unwrap();
        for token in &forbidden {
            for (line, _) in text.lines().enumerate().filter(|(_, s)| s.contains(token)) {
                hits.push(format!("{}:{}: {}", path.display(), line + 1, token));
            }
        }
    }
    assert!(
        hits.is_empty(),
        "controlled-car identity must come from ValidatorCar; forbidden references:\n{}",
        hits.join("\n")
    );
}
