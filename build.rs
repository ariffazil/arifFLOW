// build.rs — Embed build identity into the binary for runtime verification.
//
// Captures: git commit, build timestamp, dirty worktree flag.
// These are compiled into constants available at /health.
//
// Invariant: The /health endpoint MUST expose these fields so that
// repository truth can be compared to runtime truth.

use std::process::Command;

fn main() {
    // Git commit hash
    let git_commit = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    // Build timestamp (ISO 8601)
    let build_timestamp = chrono::Utc::now().to_rfc3339();

    // Dirty worktree — is the git working tree clean?
    let dirty = Command::new("git")
        .args(["diff", "--quiet"])
        .status()
        .map(|s| !s.success())
        .unwrap_or(true); // assume dirty if we can't check

    // Git branch
    let git_branch = Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    // Emit compile-time environment variables
    println!("cargo:rustc-env=GIT_COMMIT={}", git_commit);
    println!("cargo:rustc-env=BUILD_TIMESTAMP={}", build_timestamp);
    println!("cargo:rustc-env=BUILD_DIRTY={}", dirty);
    println!("cargo:rustc-env=GIT_BRANCH={}", git_branch);

    // Only rerun if git state changes
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=.git/refs/heads/");
}
