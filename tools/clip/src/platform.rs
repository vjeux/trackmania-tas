//! WHERE ffmpeg AND ffprobe ARE, decided once, explicitly.
//!
//! Two supported configurations, and nothing in between:
//!
//!   * `native`  -- a Linux or Mac ffmpeg/ffprobe on PATH, ordinary paths.
//!   * `wsl`     -- the render box (WhiteStick): WSL2 Ubuntu with **no Linux
//!                  ffmpeg at all**, driving the Windows builds under `/mnt/c`.
//!
//! The shell versions each grew their own `if command -v ... elif [ -x ... ]`
//! ladder, which meant a box with a native ffprobe but no native ffmpeg would
//! silently probe with one toolchain and encode with the other, in two
//! different path universes. Here the platform is ONE value, resolved by
//! [`resolve`] from data, refusing when it cannot be sure -- and overridable
//! with `CLIP_PLATFORM=native|wsl` when the automatic answer is wrong.
//!
//! The fact that shapes everything below: **a Windows ffmpeg/ffprobe cannot
//! read a WSL path.** `/mnt/c/Users/vjeux/tm-video/x.mp4` is not a filename
//! Windows can open; `C:/Users/vjeux/tm-video/x.mp4` is the same file. So on
//! the `wsl` platform every path handed to the tools is translated, and a file
//! that is not under `/mnt/<drive>` at all (the checkout lives in the Linux
//! home, which is invisible to Windows) is STAGED onto the Windows drive first.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::fmt::parse_probe_duration;
use crate::proc::{capture, unique_suffix};

/// The render box's Windows ffmpeg build, and the directory it stages through.
pub const WIN_FF_BIN: &str = "/mnt/c/Users/vjeux/ffmpeg_extracted/ffmpeg-9.0.1-essentials_build/bin";
pub const WIN_STAGE_DIR: &str = "/mnt/c/Users/vjeux/tm-video";
/// drawtext's own spelling of a Windows font path: the drive colon is escaped
/// because `:` separates drawtext's options.
pub const WIN_FONT: &str = r"C\:/Windows/Fonts/arialbd.ttf";

/// Fonts to try for `drawtext` on a native box, in order. Debian/Ubuntu,
/// Fedora/CentOS (the layout Meta devservers use) and macOS spellings.
pub const NATIVE_FONTS: &[&str] = &[
    "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf",
    "/usr/share/fonts/dejavu-sans-fonts/DejaVuSans-Bold.ttf",
    "/usr/share/fonts/dejavu/DejaVuSans-Bold.ttf",
    "/usr/share/fonts/truetype/liberation/LiberationSans-Bold.ttf",
    "/usr/share/fonts/liberation-sans/LiberationSans-Bold.ttf",
    "/usr/share/fonts/gnu-free/FreeSansBold.ttf",
    "/System/Library/Fonts/Supplemental/Arial Bold.ttf",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FfKind {
    Native,
    WindowsExe,
}

/// The resolved toolchain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ff {
    pub kind: FfKind,
    pub ffmpeg: PathBuf,
    pub ffprobe: PathBuf,
    /// Where a file gets copied so a Windows tool can see it. `wsl` only.
    pub stage_dir: PathBuf,
    /// A drawtext-spelled font path, or `None` when this box has no usable
    /// font -- `split` refuses in that case rather than drawing no labels.
    pub font: Option<String>,
}

/// Everything [`resolve`] is allowed to look at: gathered impurely by
/// [`from_env`], supplied by hand in the tests.
#[derive(Debug, Default, Clone)]
pub struct Inputs {
    /// `CLIP_PLATFORM`: `native`, `wsl`, or unset for automatic.
    pub want: Option<String>,
    pub env_ffmpeg: Option<PathBuf>,
    pub env_ffprobe: Option<PathBuf>,
    pub env_font: Option<String>,
    pub env_stage_dir: Option<PathBuf>,
    /// What a PATH lookup found, if anything.
    pub path_ffmpeg: Option<PathBuf>,
    pub path_ffprobe: Option<PathBuf>,
    /// The Windows build directory, and whether both .exes are really there.
    pub win_bin: PathBuf,
    pub win_ffmpeg_exists: bool,
    pub win_ffprobe_exists: bool,
    /// The first entry of [`NATIVE_FONTS`] that exists.
    pub native_font: Option<PathBuf>,
}

/// Decide the toolchain, or say precisely what is missing.
pub fn resolve(i: &Inputs) -> Result<Ff, String> {
    let want = i.want.as_deref().map(str::trim).unwrap_or("");
    let native_possible = i.path_ffmpeg.is_some() && i.path_ffprobe.is_some();
    let win_possible = i.win_ffmpeg_exists && i.win_ffprobe_exists;

    let kind = match want {
        "" => {
            if native_possible {
                FfKind::Native
            } else if win_possible {
                FfKind::WindowsExe
            } else {
                return Err(format!(
                    "no ffmpeg toolchain: PATH has {}, and {} has {}. \
                     Set CLIP_FFMPEG/CLIP_FFPROBE, or CLIP_PLATFORM=wsl with CLIP_WINFF_BIN.",
                    describe_pair(&i.path_ffmpeg, &i.path_ffprobe),
                    i.win_bin.display(),
                    describe_exes(i.win_ffmpeg_exists, i.win_ffprobe_exists),
                ));
            }
        }
        "native" => {
            if !native_possible && (i.env_ffmpeg.is_none() || i.env_ffprobe.is_none()) {
                return Err(format!(
                    "CLIP_PLATFORM=native but PATH has {} (set CLIP_FFMPEG and CLIP_FFPROBE)",
                    describe_pair(&i.path_ffmpeg, &i.path_ffprobe)
                ));
            }
            FfKind::Native
        }
        "wsl" => {
            if !win_possible && (i.env_ffmpeg.is_none() || i.env_ffprobe.is_none()) {
                return Err(format!(
                    "CLIP_PLATFORM=wsl but {} has {}",
                    i.win_bin.display(),
                    describe_exes(i.win_ffmpeg_exists, i.win_ffprobe_exists)
                ));
            }
            FfKind::WindowsExe
        }
        other => {
            return Err(format!(
                "CLIP_PLATFORM={other:?}: expected \"native\" or \"wsl\""
            ))
        }
    };

    // Never mix universes: on `wsl` both tools are the Windows pair, on
    // `native` both are the PATH pair. An override replaces one side of the
    // pair only if it was given for that side.
    let (dflt_ffmpeg, dflt_ffprobe) = match kind {
        FfKind::Native => (
            i.path_ffmpeg.clone().unwrap_or_else(|| PathBuf::from("ffmpeg")),
            i.path_ffprobe.clone().unwrap_or_else(|| PathBuf::from("ffprobe")),
        ),
        FfKind::WindowsExe => (i.win_bin.join("ffmpeg.exe"), i.win_bin.join("ffprobe.exe")),
    };

    let font = match (&i.env_font, kind) {
        (Some(f), _) => Some(f.clone()),
        (None, FfKind::WindowsExe) => Some(WIN_FONT.to_string()),
        (None, FfKind::Native) => i
            .native_font
            .as_ref()
            .map(|p| escape_font_path(&p.to_string_lossy())),
    };

    Ok(Ff {
        kind,
        ffmpeg: i.env_ffmpeg.clone().unwrap_or(dflt_ffmpeg),
        ffprobe: i.env_ffprobe.clone().unwrap_or(dflt_ffprobe),
        stage_dir: i
            .env_stage_dir
            .clone()
            .unwrap_or_else(|| PathBuf::from(WIN_STAGE_DIR)),
        font,
    })
}

fn describe_pair(a: &Option<PathBuf>, b: &Option<PathBuf>) -> String {
    match (a, b) {
        (None, None) => "neither ffmpeg nor ffprobe".to_string(),
        (Some(_), None) => "ffmpeg but no ffprobe".to_string(),
        (None, Some(_)) => "ffprobe but no ffmpeg".to_string(),
        (Some(x), Some(y)) => format!("{} and {}", x.display(), y.display()),
    }
}

fn describe_exes(ffmpeg: bool, ffprobe: bool) -> String {
    match (ffmpeg, ffprobe) {
        (false, false) => "neither ffmpeg.exe nor ffprobe.exe".to_string(),
        (true, false) => "no ffprobe.exe".to_string(),
        (false, true) => "no ffmpeg.exe".to_string(),
        (true, true) => "both .exes".to_string(),
    }
}

/// `drawtext` splits its options on `:`, so a font path's drive colon (and a
/// Windows backslash) has to be escaped before it goes in the filtergraph.
pub fn escape_font_path(p: &str) -> String {
    p.replace('\\', "/").replace(':', r"\:")
}

/// Gather the real environment and resolve it.
pub fn from_env() -> Result<Ff, String> {
    let var = |k: &str| std::env::var(k).ok().filter(|v| !v.is_empty());
    let win_bin = var("CLIP_WINFF_BIN")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(WIN_FF_BIN));
    let i = Inputs {
        want: var("CLIP_PLATFORM"),
        env_ffmpeg: var("CLIP_FFMPEG").map(PathBuf::from),
        env_ffprobe: var("CLIP_FFPROBE").map(PathBuf::from),
        env_font: var("CLIP_FONT"),
        env_stage_dir: var("CLIP_STAGE_DIR").map(PathBuf::from),
        path_ffmpeg: which("ffmpeg"),
        path_ffprobe: which("ffprobe"),
        win_ffmpeg_exists: win_bin.join("ffmpeg.exe").is_file(),
        win_ffprobe_exists: win_bin.join("ffprobe.exe").is_file(),
        win_bin,
        native_font: NATIVE_FONTS
            .iter()
            .map(PathBuf::from)
            .find(|p| p.is_file()),
    };
    resolve(&i)
}

/// A PATH lookup, without shelling out to `which`.
pub fn which(prog: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|d| d.join(prog))
        .find(|c| c.is_file() && is_executable(c))
}

#[cfg(unix)]
fn is_executable(p: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(p)
        .map(|m| m.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}
#[cfg(not(unix))]
fn is_executable(_p: &Path) -> bool {
    true
}

/// `/mnt/c/Users/vjeux/x.mp4` -> `C:/Users/vjeux/x.mp4`.
///
/// `None` for anything not on a Windows drive: on the render box that means the
/// Linux home, which the Windows tools genuinely cannot see. Callers stage the
/// file or refuse; nobody guesses.
pub fn wsl_to_windows(p: &Path) -> Option<String> {
    let s = p.to_str()?;
    let rest = s.strip_prefix("/mnt/")?;
    let mut it = rest.splitn(2, '/');
    let drive = it.next()?;
    let tail = it.next().unwrap_or("");
    let mut c = drive.chars();
    let letter = c.next()?;
    if c.next().is_some() || !letter.is_ascii_alphabetic() {
        return None;
    }
    Some(format!("{}:/{}", letter.to_ascii_uppercase(), tail))
}

impl Ff {
    /// The spelling of `p` this toolchain can open, or a refusal that says what
    /// to do about it.
    pub fn arg_path(&self, p: &Path) -> Result<String, String> {
        match self.kind {
            FfKind::Native => Ok(p.to_string_lossy().to_string()),
            FfKind::WindowsExe => wsl_to_windows(p).ok_or_else(|| {
                format!(
                    "{} is not on a Windows drive, and a Windows ffmpeg cannot read a WSL path. \
                     Put it under /mnt/<drive>/ (e.g. {}).",
                    p.display(),
                    self.stage_dir.display()
                )
            }),
        }
    }

    /// Does this ffmpeg have `drawtext` at all?
    ///
    /// A build without libfreetype has no drawtext filter, and finds out only
    /// when the filtergraph is parsed -- after both inputs have been opened,
    /// with an error that names the whole graph and buries the reason in it.
    /// The Mac's ffmpeg is such a build, and so is the common `-static` Linux
    /// one; the render box's Windows build is not. Ask first, refuse clearly.
    pub fn has_drawtext(&self) -> bool {
        capture(Command::new(&self.ffmpeg).args(["-hide_banner", "-filters"]))
            .map(|o| o.stdout.contains(" drawtext "))
            .unwrap_or(false)
    }

    /// How long is this file, and does it play at all?
    pub fn probe_duration(&self, file: &Path) -> Result<f64, String> {
        let out = self.ffprobe_entries(file, "format=duration", None)?;
        parse_probe_duration(&out).ok_or_else(|| {
            format!("{} does not probe as playable (duration: {out:?})", file.display())
        })
    }

    /// How big is the picture?
    ///
    /// The sweep's inventory needs this to tell a split-view clip from a
    /// single-camera one without watching it: a side-by-side composition is
    /// twice as wide as it is tall, and nothing else we render is.
    pub fn probe_dims(&self, file: &Path) -> Result<(u32, u32), String> {
        let out = self.ffprobe_entries(file, "stream=width,height", Some("v:0"))?;
        let t = out.trim();
        let (w, h) = t.split_once(['x', ',']).ok_or_else(|| {
            format!("{} does not probe as video ({t:?})", file.display())
        })?;
        Ok((
            w.trim().parse().map_err(|_| format!("width {w:?}"))?,
            h.trim().parse().map_err(|_| format!("height {h:?}"))?,
        ))
    }

    /// One ffprobe call, with the staging the `wsl` platform needs.
    ///
    /// On the `wsl` platform the file is copied onto the Windows drive first
    /// when it is not already there -- ffprobe.exe cannot read `/home/vjeux/...`
    /// and answers an empty string, which reads exactly like "not playable".
    /// That staging copy is the whole reason this is not two lines, and it is
    /// here rather than in each caller so a second kind of probe cannot get it
    /// wrong.
    fn ffprobe_entries(
        &self,
        file: &Path,
        entries: &str,
        stream: Option<&str>,
    ) -> Result<String, String> {
        let staged = match self.kind {
            FfKind::WindowsExe if wsl_to_windows(file).is_none() => {
                std::fs::create_dir_all(&self.stage_dir)
                    .map_err(|e| format!("cannot create {}: {e}", self.stage_dir.display()))?;
                let ext = file
                    .extension()
                    .map(|e| format!(".{}", e.to_string_lossy()))
                    .unwrap_or_default();
                let t = self.stage_dir.join(format!("_probe_{}{}", unique_suffix(), ext));
                std::fs::copy(file, &t)
                    .map_err(|e| format!("cannot stage {} to {}: {e}", file.display(), t.display()))?;
                Some(t)
            }
            _ => None,
        };
        let target = staged.clone().unwrap_or_else(|| file.to_path_buf());
        let arg = self.arg_path(&target);
        let out = arg.and_then(|a| {
            let mut v = vec!["-v".to_string(), "error".to_string()];
            if let Some(s) = stream {
                v.push("-select_streams".into());
                v.push(s.to_string());
            }
            v.push("-show_entries".into());
            v.push(entries.to_string());
            v.push("-of".into());
            v.push("csv=p=0".into());
            v.push(a);
            capture(Command::new(&self.ffprobe).args(&v))
        });
        if let Some(t) = staged {
            let _ = std::fs::remove_file(t);
        }
        let out = out?;
        Ok(out.stdout)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wsl_inputs() -> Inputs {
        Inputs {
            win_bin: PathBuf::from(WIN_FF_BIN),
            win_ffmpeg_exists: true,
            win_ffprobe_exists: true,
            ..Default::default()
        }
    }
    fn native_inputs() -> Inputs {
        Inputs {
            path_ffmpeg: Some(PathBuf::from("/usr/bin/ffmpeg")),
            path_ffprobe: Some(PathBuf::from("/usr/bin/ffprobe")),
            win_bin: PathBuf::from(WIN_FF_BIN),
            native_font: Some(PathBuf::from(
                "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf",
            )),
            ..Default::default()
        }
    }

    #[test]
    fn native_is_preferred_when_both_are_available() {
        let mut i = native_inputs();
        i.win_ffmpeg_exists = true;
        i.win_ffprobe_exists = true;
        let ff = resolve(&i).unwrap();
        assert_eq!(ff.kind, FfKind::Native);
        assert_eq!(ff.ffprobe, PathBuf::from("/usr/bin/ffprobe"));
    }

    #[test]
    fn the_render_box_falls_to_the_windows_pair() {
        let ff = resolve(&wsl_inputs()).unwrap();
        assert_eq!(ff.kind, FfKind::WindowsExe);
        assert!(ff.ffmpeg.ends_with("ffmpeg.exe"));
        assert!(ff.ffprobe.ends_with("ffprobe.exe"));
        assert_eq!(ff.font.as_deref(), Some(WIN_FONT));
        assert_eq!(ff.stage_dir, PathBuf::from(WIN_STAGE_DIR));
    }

    #[test]
    fn half_a_native_toolchain_is_never_mixed_with_half_a_windows_one() {
        // ffprobe on PATH, no native ffmpeg, Windows build present: the shell
        // ladder took the native ffprobe and the Windows ffmpeg. Refuse.
        let mut i = wsl_inputs();
        i.path_ffprobe = Some(PathBuf::from("/usr/bin/ffprobe"));
        let ff = resolve(&i).unwrap();
        assert_eq!(ff.kind, FfKind::WindowsExe);
        assert!(ff.ffprobe.ends_with("ffprobe.exe"), "{:?}", ff.ffprobe);
    }

    #[test]
    fn nothing_anywhere_is_a_refusal_naming_both_places() {
        let e = resolve(&Inputs {
            win_bin: PathBuf::from(WIN_FF_BIN),
            ..Default::default()
        })
        .unwrap_err();
        assert!(e.contains("no ffmpeg toolchain"), "{e}");
        assert!(e.contains("neither ffmpeg nor ffprobe"), "{e}");
        assert!(e.contains("ffmpeg_extracted"), "{e}");
    }

    #[test]
    fn explicit_platform_wins_and_reports_what_is_missing() {
        let mut i = native_inputs();
        i.want = Some("wsl".into());
        let e = resolve(&i).unwrap_err();
        assert!(e.contains("CLIP_PLATFORM=wsl"), "{e}");
        assert!(e.contains("neither ffmpeg.exe nor ffprobe.exe"), "{e}");

        let mut i = wsl_inputs();
        i.want = Some("native".into());
        let e = resolve(&i).unwrap_err();
        assert!(e.contains("CLIP_PLATFORM=native"), "{e}");

        let mut i = wsl_inputs();
        i.want = Some("windows".into());
        assert!(resolve(&i).unwrap_err().contains("expected"));
    }

    #[test]
    fn overrides_replace_only_the_side_they_name() {
        let mut i = wsl_inputs();
        i.env_ffprobe = Some(PathBuf::from("/opt/ffprobe"));
        i.env_stage_dir = Some(PathBuf::from("/mnt/d/stage"));
        let ff = resolve(&i).unwrap();
        assert_eq!(ff.ffprobe, PathBuf::from("/opt/ffprobe"));
        assert!(ff.ffmpeg.ends_with("ffmpeg.exe"));
        assert_eq!(ff.stage_dir, PathBuf::from("/mnt/d/stage"));
    }

    #[test]
    fn a_native_box_with_no_font_has_no_font() {
        let mut i = native_inputs();
        i.native_font = None;
        assert_eq!(resolve(&i).unwrap().font, None);
        // and one that has one gets it escaped for drawtext
        let ff = resolve(&native_inputs()).unwrap();
        assert_eq!(
            ff.font.as_deref(),
            Some("/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf")
        );
    }

    #[test]
    fn drive_letters_map_to_windows_spellings() {
        assert_eq!(
            wsl_to_windows(Path::new("/mnt/c/Users/vjeux/tm-video/a.mp4")).as_deref(),
            Some("C:/Users/vjeux/tm-video/a.mp4")
        );
        assert_eq!(
            wsl_to_windows(Path::new("/mnt/d/x")).as_deref(),
            Some("D:/x")
        );
        // the Linux home is not visible to a Windows binary, at any spelling
        assert_eq!(wsl_to_windows(Path::new("/home/vjeux/clip.mp4")), None);
        assert_eq!(wsl_to_windows(Path::new("/mnt/wsl/share/x")), None);
    }

    #[test]
    fn arg_path_refuses_a_wsl_path_on_the_windows_toolchain() {
        let ff = resolve(&wsl_inputs()).unwrap();
        let e = ff.arg_path(Path::new("/home/vjeux/clip.mp4")).unwrap_err();
        assert!(e.contains("cannot read a WSL path"), "{e}");
        assert_eq!(
            ff.arg_path(Path::new("/mnt/c/x/clip.mp4")).unwrap(),
            "C:/x/clip.mp4"
        );
        // a native toolchain passes paths through untouched
        let nf = resolve(&native_inputs()).unwrap();
        assert_eq!(
            nf.arg_path(Path::new("/home/vjeux/clip.mp4")).unwrap(),
            "/home/vjeux/clip.mp4"
        );
    }

    #[test]
    fn font_paths_are_escaped_for_drawtext() {
        assert_eq!(
            escape_font_path(r"C:\Windows\Fonts\arialbd.ttf"),
            r"C\:/Windows/Fonts/arialbd.ttf"
        );
    }
}
