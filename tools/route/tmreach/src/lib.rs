//! `tmreach` — the reachability generator of the route project (GEN arm).
//!
//! Savestates along human runs, a gate-crossing detector the plain oracle
//! agrees with, a macro fan-out from every savestate, and the `TMR0` dataset
//! the reachability model trains on. Times print as seconds with a decimal.

pub mod gates;
pub mod rig;
pub mod starts;
pub mod tele;

/// Milliseconds as seconds with a decimal (`23.144`).
pub fn secs(ms: i64) -> String {
    let neg = ms < 0;
    let a = ms.abs();
    format!("{}{}.{:03}", if neg { "-" } else { "" }, a / 1000, a % 1000)
}
pub mod gatecal;
pub mod pool;
