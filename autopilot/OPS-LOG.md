# Operations log

Recurring failures and their fixes, so a future session does not rediscover
them. Newest at the top. Each entry: **what broke**, **how it presented**,
**the fix**. If it cost more than ten minutes, it belongs here.

---

## `INFINITY` through a comparison makes every test false

A comparison against `INFINITY` is false in both directions, so a filter built
on one accepts nothing and a "best so far" seeded with one is never beaten —
and the code returns whatever came first, with no error anywhere. Seed with an
`Option` and let the type system make the empty case explicit.

## A constant steer channel makes the shim lock onto the wrong memory

The shim finds the input array by looking for the bytes it expects to be
changing. Drive with a steer value that never changes and it locks onto some
other region that happens to match, then reports confidently about it.
Vary the channel, or pin the array by identity rather than by content.

## The validator simulates until the *declared* time, not until the tape ends

A tape longer than its declared result is truncated silently; a tape shorter
than it runs past its own end. Either way the verdict is about a run nobody
asked for. Always set the declared time from the tape you are actually
submitting.

## cargo does not inherit the shell proxy

`https_proxy` in the environment gets the crates.io *index* nowhere: cargo
retries three times and dies on a random crate, which reads as a flaky network.
Write `~/.cargo/config.toml` with `[http] proxy` and `[net] git-fetch-with-cli`.
`SETUP.md` has the exact file.

## Clone into /tmp, never into ~/persistent

A clone into the persistent mount dies with `premature end of pack file` at a
different byte count every time. Three agents have tuned postBuffer, HTTP/2 and
`--filter` chasing it. Work in `/tmp`; bank to persistent and to the repo.
