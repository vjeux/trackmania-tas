# The uploader's own GitHub session — no more cookies from vjeux

vjeux, 2026-09-10: *"is there any way you can fix it so you don't need my cookie
all the time?"* Yes. This is the design, the one-time step, and what runs on
the render box afterwards.

## Why the cookie died every hour (the facts this design is built on)

Eight browser sessions were copied out of vjeux's browser on 2026-09-08/09.
Each was a `Cookie:` header pasted into `/home/vjeux/.gh-upload/cookie` and
replayed verbatim by `curl` (`ghvid.sh`). What killed them, measured:

1. **Parallel replays.** A watcher released a queue of ship scripts at once;
   6–12 `curl`s replayed one rotating session in the same second and GitHub
   answered with a `Set-Cookie` deleting `user_session` — logged out.
   (Fixed 05633743: one client under one lock, probe included.)
2. **A curl cookie JAR** (`-c`) written back over the credential file: curl keeps
   only cookies with an expiry, so the round trip dropped `user_session`,
   `__Host-user_session_same_site`, `logged_in`, `dotcom_user`, `_gh_sess` and
   left 33 bytes of `_octo`. A second jar attempt silently dropped the
   `__Host-` cookie and sent incomplete sessions. (Fixed: the file is INPUT.)
3. **A datacenter IP.** One page fetch with the header from a devserver through
   fwdproxy and GitHub logged the session out everywhere within minutes.
4. **vjeux signing in again** to mint the next header — a fresh login in the
   same browser rotates/invalidates what the uploader was holding.

And the control: on 2026-09-08 ONE header published 20 clips untouched, ~10 min
apart, sequentially, from the box. Sequential replays of a static header work;
GitHub does not require the client to follow `_gh_sess` rotation. So the
session does not need a browser to stay alive — it needs **one client, one
IP, no logins beside it, and a jar that keeps every cookie the server sets.**

## What GitHub's login requires, and what that means for an automated profile

- The account has 2FA (GitHub requires it for contributors since 2023). A login
  = password + TOTP / passkey / GitHub Mobile prompt. There is **no device-code
  flow for a browser session** (device flow exists only for OAuth apps and mints
  API tokens, which cannot use the private `/upload/policies/assets` endpoint —
  that endpoint is what puts a video behind `github.com/user-attachments/…`).
  So the one-time step is a human login, once.
- After login GitHub sets `user_session` and `__Host-user_session_same_site`
  (the session; cookie lifetime two weeks, **renewed while the session is
  used** — a browser left signed in stays signed in for months), `logged_in`,
  `dotcom_user`, `_device_id` (one year; marks the device as seen so the next
  login on it skips the "verify device" mail), `_gh_sess` (the Rails session,
  rotated per response — old values stay accepted), `_octo`, and a few
  preferences. A jar that stores ALL of these, including the ones without an
  expiry, and applies every `Set-Cookie` it is sent, is a browser as far as the
  session is concerned.
- What ends a session: signing out, "revoke all sessions", a password change,
  two weeks of no use, and the abuse heuristics in the list above (parallel
  replays, IP hopping). None of these happens to a jar that lives on the box,
  is used sequentially, and is exercised once a day.

**Therefore: not a headless browser.** A persistent Chromium profile (Playwright
on the Windows side) was the first idea and it is the wrong tool here: it needs
Node or Python on the box (the box has neither Node nor a Rust Windows
toolchain; a WSL controller cannot reach a Windows Chrome's DevTools port), it
drives GitHub's ever-changing editor UI, and it adds nothing the jar does not
already do — the session's survival is a property of how it is USED, which the
jar controls completely. The uploader gets a jar of its own, fed once by a
login that is then never used by anything else.

## The design: `ghsession` (tools/ghsession, Rust, runs on the box in WSL)

One binary, one jar file `/home/vjeux/.gh-upload/session.json` (mode 600,
written atomically; every cookie GitHub sets, expiry or not, with its domain,
path and flags). Every command sends the jar's cookies, applies every
`Set-Cookie` from every response, and saves — so rotation is followed, and the
jar is always the latest state of the session.

```
ghsession seed --cookie-header FILE     ONE-TIME: adopt a fresh login as the uploader's session
ghsession status                        GET github.com/vjeux/trackmania-tas: "logged in as vjeux" or exit 3
ghsession keepalive                     status + two ordinary page views (cron, daily)
ghsession upload FILE [--content-type]  the three-request user-attachments upload; prints the asset URL
```

- `upload` is byte-for-byte the protocol `ghvid.sh` proved (a CSRF token off the
  README edit page, `POST /upload/policies/assets`, the S3 form post, `PUT
  /upload/assets/<id>`), with the browser headers `ghvid.sh` sends, and a normal
  page view of the repo afterwards. It prints the URL on stdout and nothing
  else, so `clip ship` runs it in place of `ghvid.sh` (`GHVID=…/ghsession-upload.sh`).
- `status` answers from the page's `<meta name="user-login">`; a 302 to
  `/login` is "logged out", exit 3, and it says so in one line. Nothing retries
  a dead session: the ship script stops (as today) and the coordinator is told.
- **Never two clients.** `tinyship.sh` already serialises probe → upload → page
  view → cooldown under `/home/vjeux/shoot/tinyship.lock`; the daily keepalive
  runs under the same lock (`flock /home/vjeux/shoot/tinyship.lock ghsession
  keepalive`), so it can never overlap a ship. The credential file
  `~/.gh-upload/cookie` is not read by anything once the jar exists.
- The devserver side never touches the session (rule 3 above): `ghsession`
  refuses to run when `/home/vjeux/.gh-upload/session.json` is not on the box
  it was seeded on (a `host` field in the jar; today: `WhiteStick`).

### The one-time step for vjeux (five minutes, once)

1. On the Mac, open a **private/incognito window**, go to https://github.com/login,
   sign in (password + 2FA), tick nothing special. This is a NEW session, separate
   from the browser's normal one.
2. Open https://github.com/vjeux/trackmania-tas in that window, DevTools →
   Network → click the document request → copy the `Cookie:` request header
   (one line, ~1.2 KB).
3. Put it where the coordinator asks (today: the coordinator writes it to the
   box as `/home/vjeux/.gh-upload/seed.txt`, mode 600, and runs
   `ghsession seed --cookie-header /home/vjeux/.gh-upload/seed.txt`, which
   verifies it with one `status` request and deletes the seed file).
4. **Close the private window without signing out.** From this moment that
   session belongs to the uploader; vjeux's own browser session is never
   touched again, and he may use GitHub normally at any time.

Alternative one-time step, if he prefers not to copy a header: sign in on the
box itself in Edge (a browser on the box's screen), then export that window's
`Cookie:` header the same way — same result, and the header never leaves the
machine.

### What keeps it alive

- `ghsession keepalive` from the box's crontab once a day (`0 9 * * *`, under
  the ship lock): one authenticated page view is what a person who leaves a tab
  open generates. Every ship is activity as well.
- GitHub's two-week session lifetime is measured from the last use. The jar is
  used at least daily, so the session lasts until the account revokes it. If
  GitHub ever ends it anyway (a password change, "sign out everywhere"),
  `status` says `logged out` on the next ship, the ship stops, and the one-time
  step is repeated — at worst once, not once an hour.

### Failure modes and how each shows

| symptom | meaning | what happens |
|---|---|---|
| `status`: `logged out (302 → /login)` | the session ended | ship STOPS with `FAILED cookie probe`; coordinator told; repeat the one-time step |
| `upload`: `no upload CSRF token` (exit 3) | logged out between probe and upload | same |
| `upload`: step 1 returned 4xx | GitHub refused the policy (rate, abuse) | ship FAILED, nothing uploaded; wait an hour |
| jar file missing | never seeded on this box | `tinyship.sh` falls back to `~/.gh-upload/cookie` (the old path) so nothing silently changes |

### Files

- `tools/ghsession/` — the crate (`cargo build --release -p ghsession`; built on the
  box with `tinyctl box-build --crates ghsession`).
- `tools/tinyctl/box/ghsession-upload.sh` — the `GHVID` shim: `exec ghsession upload "$1"`.
- `tools/tinyctl/box/tinyship.sh` — probes with `ghsession status` and ships
  with the shim when the jar exists; unchanged behaviour otherwise.
- `/home/vjeux/.gh-upload/session.json` — the jar (box only, mode 600, never in the repo, never on the store).
