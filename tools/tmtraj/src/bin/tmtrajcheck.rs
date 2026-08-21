// tmtrajcheck -- `tmtraj check`, the refuse-to-publish gate, as a standalone
// binary in this tree.
//
// The upstream build (whl_tools_v1/build.sh) reconstructs a workspace from five
// base tarballs under ~/persistent/private-30d, which live on the TM boxes and
// not on WhiteStick. But checkcmd.rs needs only whlcmd.rs, and whlcmd.rs needs
// only four functions from entrec -- all four of which this tree already
// exports. So the gate is dropped in unmodified and wrapped here, rather than
// reimplemented (a lookalike that disagreed at the margins would be worse than
// no gate).
//
//   tmtrajcheck GHOST.Ghost.Gbx --race <validated_ms>
//   exit 0 = publishable, 1 = warnings, 2 = REFUSED
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    tmtraj::checkcmd::cmd(&args);
}
