//! Chooses which icon fonts to embed: the small built-in subsets, the full packs (features
//! `icons-lucide-full` and `icons-phosphor-full`), or a tree-shaken pack produced by
//! `tools/icons/shake.py` and named by the `ALLOY_ICONS_SUBSET` environment variable.

use std::{env, fs, path::PathBuf};

fn main() {
    let out = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));
    let fonts = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("manifest dir")).join("fonts");
    println!("cargo:rerun-if-env-changed=ALLOY_ICONS_SUBSET");
    println!("cargo:rerun-if-changed=fonts");
    let shaken = env::var_os("ALLOY_ICONS_SUBSET").map(PathBuf::from);
    for (name, small, full, feature) in [
        (
            "Lucide.ttf",
            "Lucide-subset.ttf",
            "full/Lucide-full.ttf",
            "CARGO_FEATURE_ICONS_LUCIDE_FULL",
        ),
        (
            "Phosphor.ttf",
            "Phosphor-subset.ttf",
            "full/Phosphor-full.ttf",
            "CARGO_FEATURE_ICONS_PHOSPHOR_FULL",
        ),
    ] {
        let from = match &shaken {
            Some(dir) if dir.join(name).exists() => dir.join(name),
            _ if env::var_os(feature).is_some() => fonts.join(full),
            _ => fonts.join(small),
        };
        fs::copy(&from, out.join(name)).unwrap_or_else(|e| panic!("copy {}: {e}", from.display()));
    }
}
