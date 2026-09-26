# credread — read Chrome-saved passwords on this Windows machine

Built for a one-off: recover vjeux's Trackmania-related site passwords
(`connect.ubisoft.com`, `www.maniaplanet.com`) from his WhiteStick box to
create a TM2020 dedicated-server account.

## How it works

- **v10 entries**: classic DPAPI. `CryptUnprotectData` (user scope) unwraps
  `os_crypt.encrypted_key` (minus the `DPAPI` prefix) into the AES-256-GCM
  key; each blob is `v10‖nonce(12)‖ct‖tag(16)`, decrypted with Windows CNG.
- **v20 entries**: app-bound. The `APPB`-prefixed blob in
  `os_crypt.app_bound_encrypted_key` only unwraps inside Chrome's elevation
  service (`GoogleChromeElevationService`) via `IElevator::DecryptData`, and
  the service validates that the *calling process path* matches the path
  that encrypted it (`C:\Program Files\Google\Chrome\...`). So:
  - `v20direct` calls the service from this exe (expected to fail with
    `kValidationDidNotPass` — useful only to prove the COM plumbing works);
  - `v20inject` launches a headless, GPU-disabled `chrome.exe`, injects
    `payload.dll` into it with `CreateRemoteThread + LoadLibraryA`, the
    payload makes the COM call (path validation passes) and writes the key
    to a file, then Chrome is terminated. GCM decryption happens back in
    this exe. Only the Chrome child this tool launched is ever terminated.

The tool never prints key material (`KEY_OK len=32` only) and scrubs its
workdir files on the way out. Password blobs and key blobs travel as hex on
the command line (extracted from a *copy* of Chrome's `Login Data` with the
`sqlite3` CLI — Chrome locks the original).

## Build (on the WhiteStick box WSL, which has the msvc std + rust-lld)

```
cargo run -p credread --bin xbuild [-- kernel32:SymA,SymB ...]
```

This generates MSVC import libs with no SDK (`mkimplib.rs`, same recipe as
the January probe) and cross-links `target/xbuild/credread.exe` and
`target/xbuild/payload.dll`. The fixpoint loop auto-adds known symbols on
undefined-symbol errors; truly unknown symbols (or extra `dll:syms` args)
are reported/added explicitly. Zero dependencies, no network needed.

## Run (from WSL via interop)

```
OUT=C:/Users/vjeux/credread-run
./target/xbuild/credread.exe v10 <dpapi_hex> <blob_hex>...
./target/xbuild/credread.exe v20direct <appb_hex> <blob_hex>...
./target/xbuild/credread.exe v20inject 'C:\Program Files\Google\Chrome\Application\chrome.exe' ./target/xbuild/payload.dll $OUT <appb_hex> <blob_hex>...
```

Output lines: `KEY_OK len=32`, `PW0=...` (or `PW0=HEX:...`), `ERR ...`.
