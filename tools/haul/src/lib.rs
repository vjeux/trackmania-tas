//! `tmhaul` — the long-haul harness for the TM2020 autopilot.
//!
//! The project runs for months on boxes that live at most eighteen hours, with
//! no human tending it. Everything in this crate exists to make that ordinary:
//! state that lives in the repo rather than on a machine, a supervisor that is
//! a process rather than an agent's attention, alarms shaped like the failures
//! that actually happened, and a budget that counts work instead of time.

pub mod alarms;
pub mod bank;
pub mod beat;
pub mod budget;
pub mod config;
pub mod credential;
pub mod disk;
pub mod gates;
pub mod gitcmd;
pub mod ledger;
pub mod lease;
pub mod log;
pub mod md5;
pub mod pack;
pub mod paths;
pub mod queue;
pub mod rec;
pub mod recover;
pub mod state;
pub mod status;
pub mod time;
pub mod watch;
pub mod worker;
