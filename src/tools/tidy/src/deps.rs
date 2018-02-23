// Copyright 2016 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Check license of third-party deps by inspecting src/vendor

use std::fs::File;
use std::io::Read;
use std::path::Path;

use std::process::Command;

use serde_json;

static LICENSES: &'static [&'static str] = &[
    "MIT/Apache-2.0",
    "MIT / Apache-2.0",
    "Apache-2.0/MIT",
    "Apache-2.0 / MIT",
    "MIT OR Apache-2.0",
    "MIT",
    "Unlicense/MIT",
];

// These are exceptions to Rust's permissive licensing policy, and
// should be considered bugs. Exceptions are only allowed in Rust
// tooling. It is _crucial_ that no exception crates be dependencies
// of the Rust runtime (std / test).
static EXCEPTIONS: &'static [&'static str] = &[
    "mdbook",             // MPL2, mdbook
    "openssl",            // BSD+advertising clause, cargo, mdbook
    "pest",               // MPL2, mdbook via handlebars
    "thread-id",          // Apache-2.0, mdbook
    "toml-query",         // MPL-2.0, mdbook
    "is-match",           // MPL-2.0, mdbook
    "cssparser",          // MPL-2.0, rustdoc
    "smallvec",           // MPL-2.0, rustdoc
    "fuchsia-zircon-sys", // BSD-3-Clause, rustdoc, rustc, cargo
    "fuchsia-zircon",     // BSD-3-Clause, rustdoc, rustc, cargo (jobserver & tempdir)
    "cssparser-macros",   // MPL-2.0, rustdoc
    "selectors",          // MPL-2.0, rustdoc
    "clippy_lints",       // MPL-2.0 rls
];

// Whitelist of crates rustc is allowed to depend on. Avoid adding to the list if possible.
static WHITELIST: &'static [(&'static str, &'static str)] = &[];

// Some type for Serde to deserialize the output of `cargo metadata` to...

#[derive(Deserialize)]
struct Output {
    packages: Vec<Package>,
    _resolve: String,
}

#[derive(Deserialize)]
struct Package {
    _id: String,
    name: String,
    version: String,
    _source: Option<String>,
    _manifest_path: String,
}

/// Checks the dependency at the given path. Changes `bad` to `true` if a check failed.
///
/// Specifically, this checks that the license is correct and that the dependencies are on the
/// whitelist.
pub fn check(path: &Path, bad: &mut bool) {
    // Check licences
    let path = path.join("vendor");
    assert!(path.exists(), "vendor directory missing");
    let mut saw_dir = false;
    for dir in t!(path.read_dir()) {
        saw_dir = true;
        let dir = t!(dir);

        // skip our exceptions
        if EXCEPTIONS.iter().any(|exception| {
            dir.path()
                .to_str()
                .unwrap()
                .contains(&format!("src/vendor/{}", exception))
        }) {
            continue;
        }

        let toml = dir.path().join("Cargo.toml");
        *bad = *bad || !check_license(&toml);
    }
    assert!(saw_dir, "no vendored source");

    // Check dependencies
    let deps = get_deps(&path);
    *bad = *bad
        || deps.iter().any(
            |&Package {
                 ref name,
                 ref version,
                 ..
             }| {
                WHITELIST
                    .iter()
                    .all(|&(wname, wversion)| name != wname || version != wversion)
            },
        );
}

fn check_license(path: &Path) -> bool {
    if !path.exists() {
        panic!("{} does not exist", path.display());
    }
    let mut contents = String::new();
    t!(t!(File::open(path)).read_to_string(&mut contents));

    let mut found_license = false;
    for line in contents.lines() {
        if !line.starts_with("license") {
            continue;
        }
        let license = extract_license(line);
        if !LICENSES.contains(&&*license) {
            println!("invalid license {} in {}", license, path.display());
            return false;
        }
        found_license = true;
        break;
    }
    if !found_license {
        println!("no license in {}", path.display());
        return false;
    }

    true
}

fn extract_license(line: &str) -> String {
    let first_quote = line.find('"');
    let last_quote = line.rfind('"');
    if let (Some(f), Some(l)) = (first_quote, last_quote) {
        let license = &line[f + 1..l];
        license.into()
    } else {
        "bad-license-parse".into()
    }
}

fn get_deps(path: &Path) -> Vec<Package> {
    // Run `cargo metadata` to get the set of dependencies
    let output = Command::new("cargo")
        .arg("metadata")
        .arg("--format-version")
        .arg("1")
        .arg("--manifest-path")
        .arg(path.join("Cargo.toml"))
        .output()
        .expect("Unable to run `cargo metadata`")
        .stdout;
    let output = String::from_utf8_lossy(&output);
    let output: Output = serde_json::from_str(&output).unwrap();

    output.packages
}
