//! Reading `ghost verify -o json`.
//!
//! A parser for ONE shape, not a JSON library: this workspace has a single
//! third-party dependency and a serde stack is not worth becoming the second
//! for a document of the form
//!
//! ```text
//! {"checks": [{"id": "V6", "verdict": "pass", "message": "..."}, ...],
//!  "pass": 7, "fail": 1, "warn": 2, "na": 1}
//! ```
//!
//! It replaces line-scanning that split the HUMAN output on the literal
//! strings `"kappa "`, `"file: "` and `"re-simulated"` -- so rewording a gate
//! message silently turned a real audit into "unchecked".
//!
//! Anything it cannot understand yields an empty list, and the caller reports
//! that as an unchecked page rather than inventing a verdict.

/// Every check in the report, as `(id, verdict, message)`.
pub fn checks(s: &str) -> Vec<(String, String, String)> {
    let mut out = Vec::new();
    let Some(start) = s.find("\"checks\"") else { return out };
    let rest = &s[start..];
    let Some(open) = rest.find('[') else { return out };
    // Brace counting must IGNORE braces inside strings. Gate messages carry
    // real punctuation -- `declared 19.538, found {19.539}` -- and counting
    // those as structure ends the object early and truncates the message.
    // Escapes are honoured so a `\"` inside a message does not close it
    // either.
    let mut depth = 0usize;
    let mut in_str = false;
    let mut escaped = false;
    let mut obj = String::new();
    for ch in rest[open..].chars() {
        if in_str {
            obj.push(ch);
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_str = false;
            }
            continue;
        }
        match ch {
            '"' => {
                in_str = true;
                obj.push(ch);
            }
            '{' => {
                depth += 1;
                obj.clear();
            }
            '}' if depth > 0 => {
                depth -= 1;
                if let (Some(i), Some(v)) = (field(&obj, "id"), field(&obj, "verdict")) {
                    out.push((i, v, field(&obj, "message").unwrap_or_default()));
                }
            }
            ']' if depth == 0 => break,
            c if depth > 0 => obj.push(c),
            _ => {}
        }
    }
    out
}

/// The string value of `"name"` in one flat object body, un-escaping the
/// sequences `Report::json` emits.
fn field(obj: &str, name: &str) -> Option<String> {
    let key = format!("\"{}\"", name);
    let at = obj.find(&key)? + key.len();
    let rest = &obj[at..];
    let colon = rest.find(':')? + 1;
    let rest = &rest[colon..];
    let q = rest.find('"')? + 1;
    let body = &rest[q..];
    let mut out = String::new();
    let mut it = body.chars();
    while let Some(c) = it.next() {
        match c {
            '"' => return Some(out),
            '\\' => match it.next() {
                Some('n') => out.push('\n'),
                Some('t') => out.push('\t'),
                Some('"') => out.push('"'),
                Some('\\') => out.push('\\'),
                Some('u') => {
                    let hex: String = it.by_ref().take(4).collect();
                    if let Some(c) = u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32) {
                        out.push(c);
                    }
                }
                Some(other) => out.push(other),
                None => break,
            },
            c => out.push(c),
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"{"checks": [{"id": "V1", "verdict": "pass", "message": "codec identity: ok"}, {"id": "V6", "verdict": "pass", "message": "kappa 1.000 over 391 samples"}, {"id": "V7", "verdict": "na", "message": "no dedicated server"}], "pass": 2, "fail": 0, "warn": 0, "na": 1}"#;

    #[test]
    fn reads_every_check() {
        let c = checks(SAMPLE);
        assert_eq!(c.len(), 3);
        assert_eq!(c[0].0, "V1");
        assert_eq!(c[1].1, "pass");
        assert_eq!(c[2].2, "no dedicated server");
    }

    #[test]
    fn the_number_a_caller_wants_survives_the_round_trip() {
        let c = checks(SAMPLE);
        let v6 = c.iter().find(|(id, _, _)| id == "V6").unwrap();
        let k: f64 = v6.2.split("kappa ").nth(1).unwrap().split_whitespace().next().unwrap()
            .parse().unwrap();
        assert!((k - 1.0).abs() < 1e-9);
    }

    #[test]
    fn a_message_containing_punctuation_does_not_end_the_object_early() {
        // Gate messages carry commas, colons, braces and quoted paths. If the
        // scanner treated any of those as structure it would truncate.
        let s = r#"{"checks": [{"id": "V2", "verdict": "fail", "message": "declared 19.538, found {19.539}: \"foo.Ghost.Gbx\""}], "pass": 0, "fail": 1, "warn": 0, "na": 0}"#;
        let c = checks(s);
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].0, "V2");
        assert!(c[0].2.contains("{19.539}"), "message truncated: {:?}", c[0].2);
        assert!(c[0].2.contains("\"foo.Ghost.Gbx\""), "escapes lost: {:?}", c[0].2);
    }

    #[test]
    fn garbage_yields_nothing_rather_than_a_wrong_answer() {
        assert!(checks("").is_empty());
        assert!(checks("not json at all").is_empty());
        assert!(checks("{\"checks\": [").is_empty());
    }
}
