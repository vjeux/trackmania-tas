// entry — the PE entry point for the cross-linked .exe (the console subsystem
// defaults to mainCRTStartup, which the real CRT normally provides; this box has
// no Windows SDK, so we bring our own — the credread/January recipe).
//
// rustc's generated `main` would run std's runtime init; in this shape we skip it
// and call `idle_main` directly (it reads argv from the OS itself).

extern "C" {
    fn ExitProcess(code: u32) -> !;
}

extern "Rust" {
    fn idle_main();
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mainCRTStartup() -> ! {
    idle_main();
    ExitProcess(0)
}
