//! Behavioural tests for the compiled binary: argument handling, config
//! layering and commands that do not need a registry.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn npmr() -> Command {
    Command::new(env!("CARGO_BIN_EXE_npmr"))
}

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("npmr-test-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

fn write(path: &Path, contents: &str) {
    std::fs::create_dir_all(path.parent().expect("parent")).expect("create dirs");
    std::fs::write(path, contents).expect("write file");
}

fn read_json(path: &Path) -> serde_json::Value {
    let body = std::fs::read_to_string(path).expect("read json");
    serde_json::from_str(&body).expect("valid json")
}

fn run_in(dir: &Path, args: &[&str]) -> Output {
    npmr()
        .current_dir(dir)
        .args(args)
        .output()
        .expect("run npmr")
}

fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

struct Project {
    dir: PathBuf,
}

impl Project {
    fn new(name: &str) -> Self {
        Self {
            dir: temp_dir(name),
        }
    }

    fn file(self, rel: &str, contents: &str) -> Self {
        write(&self.dir.join(rel), contents);
        self
    }

    fn run(&self, args: &[&str]) -> Output {
        run_in(&self.dir, args)
    }
}

impl Drop for Project {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

#[test]
fn version_flag_prints_crate_version() {
    let cwd = std::env::temp_dir();
    let out = run_in(&cwd, &["--version"]);
    assert!(out.status.success());
    assert_eq!(stdout(&out).trim(), env!("CARGO_PKG_VERSION"));
}

#[test]
fn help_lists_commands() {
    let cwd = std::env::temp_dir();
    let out = run_in(&cwd, &["--help"]);
    let text = stdout(&out);
    for command in [
        "access", "config", "install", "run", "stage", "trust", "whoami",
    ] {
        assert!(text.contains(command), "usage is missing {command}");
    }
}

#[test]
fn unknown_command_is_reported_with_misspelling_suggestion() {
    let cwd = std::env::temp_dir();
    let out = run_in(&cwd, &["instaL"]);
    assert!(!out.status.success());
    let text = stderr(&out);
    assert!(text.contains("Unknown command: \"instaL\""), "{text}");
    assert!(text.contains("npm install"), "missing suggestion: {text}");
}

#[test]
fn unknown_command_has_no_suggestions_for_gibberish() {
    let cwd = std::env::temp_dir();
    let out = run_in(&cwd, &["frobnicate"]);
    assert!(!out.status.success());
    let text = stderr(&out);
    assert!(text.contains("Unknown command: \"frobnicate\""), "{text}");
    assert!(!text.contains("Did you mean"), "{text}");
}

#[test]
fn misspelled_commands_still_resolve() {
    let project = Project::new("typo");
    let out = project.run(&["ins", "--version"]);
    assert!(out.status.success(), "{}", stderr(&out));
}

#[test]
fn non_ascii_dash_argument_is_rejected() {
    let cwd = std::env::temp_dir();
    let out = run_in(&cwd, &["prefix", "–save"]);
    let text = stderr(&out);
    assert!(text.contains("non-ascii dash"), "{text}");
}

#[test]
fn registry_dependent_command_reports_enotsup() {
    let cwd = std::env::temp_dir();
    let out = run_in(&cwd, &["view", "lodash"]);
    assert!(!out.status.success());
    assert!(stderr(&out).contains("ENOTSUP"));
}

#[test]
fn project_npmrc_is_layered_below_the_cli() {
    let project = Project::new("npmrc").file(".npmrc", "loglevel=warn\n");
    let from_file = project.run(&["config", "get", "loglevel"]);
    assert!(from_file.status.success());
    assert_eq!(stdout(&from_file).trim(), "warn");

    let from_cli = project.run(&["config", "get", "loglevel", "--loglevel", "error"]);
    assert_eq!(stdout(&from_cli).trim(), "error");
}

#[test]
fn config_fix_renames_underscore_keys() {
    let project = Project::new("fix").file(".npmrc", "prefer_online=true\n");
    let out = project.run(&["config", "fix"]);
    assert!(out.status.success(), "{}", stderr(&out));
    let body = std::fs::read_to_string(project.dir.join(".npmrc")).expect("read npmrc");
    assert!(body.contains("prefer-online"), "{body}");
    assert!(!body.contains("prefer_online"), "{body}");
}

#[test]
fn pkg_get_reads_manifest_fields() {
    let project = Project::new("pkg-get").file(
        "package.json",
        r#"{"name":"demo","version":"1.2.3","repository":{"url":"https://example.com/demo"}}"#,
    );
    let out = project.run(&["pkg", "get", "name", "version", "--json=true"]);
    assert!(out.status.success(), "{}", stderr(&out));
    let parsed: serde_json::Value =
        serde_json::from_str(&stdout(&out)).expect("pkg get emits JSON");
    assert_eq!(parsed["name"], "demo");
    assert_eq!(parsed["version"], "1.2.3");

    let nested = project.run(&["pkg", "get", "repository.url", "--json=true"]);
    let parsed: serde_json::Value =
        serde_json::from_str(&stdout(&nested)).expect("nested pkg get emits JSON");
    assert_eq!(parsed["repository.url"], "https://example.com/demo");
}

#[test]
fn pkg_set_and_delete_round_trip() {
    let project = Project::new("pkg-set").file("package.json", r#"{"name":"demo"}"#);
    let set = project.run(&["pkg", "set", "scripts.test=tap", "description=hi"]);
    assert!(set.status.success(), "{}", stderr(&set));
    let manifest =
        std::fs::read_to_string(project.dir.join("package.json")).expect("read manifest");
    let parsed: serde_json::Value = serde_json::from_str(&manifest).expect("valid manifest");
    assert_eq!(parsed["scripts"]["test"], "tap");
    assert_eq!(parsed["description"], "hi");

    let delete = project.run(&["pkg", "delete", "scripts.test"]);
    assert!(delete.status.success(), "{}", stderr(&delete));
    let parsed = read_json(&project.dir.join("package.json"));
    assert!(parsed["scripts"].get("test").is_none());
}

#[test]
fn pkg_get_on_missing_manifest_fails_with_ejsonparse() {
    let project = Project::new("pkg-missing");
    let out = project.run(&["pkg", "get", "name"]);
    assert!(!out.status.success());
    let text = stderr(&out);
    assert!(text.contains("Could not read package.json"), "{text}");
    assert!(text.contains("EJSONPARSE"), "{text}");
}

#[test]
fn ls_lists_direct_dependencies() {
    let project = Project::new("ls")
        .file("package.json", r#"{"name":"demo","version":"1.0.0"}"#)
        .file(
            "node_modules/foo/package.json",
            r#"{"name":"foo","version":"2.0.0"}"#,
        )
        .file(
            "node_modules/@scope/bar/package.json",
            r#"{"name":"@scope/bar","version":"0.1.0"}"#,
        );
    let out = project.run(&["ls"]);
    assert!(out.status.success(), "{}", stderr(&out));
    let text = stdout(&out);
    assert!(text.contains("demo@1.0.0"), "{text}");
    assert!(text.contains("foo@2.0.0"), "{text}");
    assert!(text.contains("@scope/bar@0.1.0"), "{text}");
}

#[test]
fn prefix_prints_the_local_prefix() {
    let project = Project::new("prefix");
    let out = project.run(&["prefix"]);
    assert!(out.status.success(), "{}", stderr(&out));
    let printed = PathBuf::from(stdout(&out).trim());
    assert_eq!(
        std::fs::canonicalize(printed).unwrap(),
        std::fs::canonicalize(&project.dir).unwrap()
    );
}

#[test]
fn whoami_without_auth_fails_with_eneedauth() {
    let project = Project::new("whoami").file("package.json", r#"{"name":"demo"}"#);
    let out = project.run(&["whoami"]);
    assert!(!out.status.success());
    let text = stderr(&out);
    assert!(text.contains("ENEEDAUTH"), "{text}");
}

#[test]
fn init_writes_a_manifest_from_arguments() {
    let project = Project::new("init");
    let ok = project.run(&["init", "--yes"]);
    assert!(ok.status.success(), "{}", stderr(&ok));
    let parsed = read_json(&project.dir.join("package.json"));
    assert!(parsed.get("name").is_some(), "{parsed}");
}
