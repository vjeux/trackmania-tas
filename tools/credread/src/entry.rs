// entry — the PE entry point for the .exe (console subsystem defaults to
// mainCRTStartup, which the real CRT normally provides).
//
// The real CRT parses the command line, initialises stdio and locale, then
// calls main. Rust's std on Windows reads the command line itself through
// GetCommandLineW, and rustc's generated `main` runs the runtime init
// (lang_start), so the entry only has to call it and exit with its code.
// argc/argv are ignored by the Windows runtime init; dummy values are fine.

extern "C" {
    fn ExitProcess(code: u32) -> !;
}

extern "Rust" {
    fn cred_main();
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mainCRTStartup() -> ! {
    cred_main();
    ExitProcess(0)
}
