use std::{env, fs};

fn require(text: &str, needle: &str, label: &str) {
    assert!(text.contains(needle), "missing {label}: {needle}");
}

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.len() != 2 {
        eprintln!("usage: control_verify PRE_OUTPUT POST_OUTPUT");
        std::process::exit(2);
    }
    let pre = fs::read_to_string(&args[0]).expect("read pre-boundary output");
    let post = fs::read_to_string(&args[1]).expect("read post-boundary output");

    require(&pre, "\"Time\" : -1", "pre-boundary failed time");
    require(&pre, "\"Desc\" : \"wrong simu\\n\"", "pre-boundary wrong-simulation result");
    require(&pre, "\"IsValid\" : false", "pre-boundary invalid result");
    require(&pre, "\"Time\" : 63546", "pre-boundary declared time");

    require(&post, "\"NbCheckpoints\" : 5", "post-boundary checkpoints");
    require(&post, "\"Time\" : 63546", "post-boundary validated time");
    require(&post, "\"IsValid\" : true", "post-boundary valid result");
    assert!(!post.contains("wrong simu"), "post-boundary run reported wrong simulation");

    println!("authoritative server control verified: March 25 WRONG_SIMU; March 29 exact 63.546");
}
