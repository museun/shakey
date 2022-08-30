use std::{path::PathBuf, process::Command};

use time::{format_description::well_known::Rfc2822, OffsetDateTime};

fn main() {
    std::fs::write(
        PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("version.rs"),
        indoc::formatdoc!(
            r#"
                pub const GIT_BRANCH: &str = "{git_branch}";
                pub const GIT_REVISION: &str = "{git_revision}";
                pub const BUILD_TIME: &str = "{build_time}";
            "#,
            git_branch = get_branch().expect("should be in git"),
            git_revision = get_revision().expect("should be in git"),
            build_time = current_time(),
        ),
    )
    .unwrap()
}

fn current_time() -> String {
    OffsetDateTime::now_local()
        .unwrap()
        .format(&Rfc2822)
        .unwrap()
}

fn get_branch() -> Option<String> {
    get_git(Some("--abbrev-ref"))
}

fn get_revision() -> Option<String> {
    get_git(None).map(|s| s[..10].to_string())
}

fn get_git(flag: Option<&str>) -> Option<String> {
    let mut cmd = Command::new("git");
    cmd.arg("rev-parse");
    if let Some(flag) = flag {
        cmd.arg(flag);
    };
    let out = cmd.arg("@").output().ok()?.stdout;

    std::str::from_utf8(&out)
        .ok()
        .map(<str>::trim)
        .map(ToString::to_string)
}
