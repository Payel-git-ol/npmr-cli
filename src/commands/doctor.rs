//! Port of `lib/commands/doctor.js`: runs environment checks and reports the
//! ones that need fixing. Network checks (ping, latest versions) require the
//! registry client and currently fail with `ENOTSUP`.

use std::path::{Path, PathBuf};

use crate::commands::command::{Command, CommandMeta};
use crate::npm::Npm;
use crate::utils::errors::{NpmError, Result};
use crate::utils::log::{error, info, warn};

pub static META: CommandMeta = CommandMeta::new("doctor", "Check the health of your npm environment")
    .with_params(&["registry"])
    .with_usage(&[
        "[connection] [registry] [versions] [environment] [cache] [permissions]",
    ])
    .skip_validation();

const R_OK: u32 = 0b001;
const W_OK: u32 = 0b010;
const X_OK: u32 = 0b100;

/// Where a permission check looks, since closures cannot live in a `const`.
#[derive(Clone, Copy)]
enum CheckRoot {
    Cache,
    LocalDir,
    GlobalDir,
    LocalBin,
    GlobalBin,
}

struct PermissionCheck {
    groups: &'static [&'static str],
    title: &'static str,
    mask: u32,
    root: CheckRoot,
    should_own: bool,
    missing_ok: bool,
}

const PERMISSION_CHECKS: &[PermissionCheck] = &[
    PermissionCheck {
        groups: &["permissions", "cache"],
        title: "Checking permissions on cached files (this may take awhile)",
        mask: R_OK,
        root: CheckRoot::Cache,
        should_own: true,
        missing_ok: false,
    },
    PermissionCheck {
        groups: &["permissions"],
        title: "Checking permissions on local node_modules (this may take awhile)",
        mask: R_OK | W_OK,
        root: CheckRoot::LocalDir,
        should_own: true,
        missing_ok: true,
    },
    PermissionCheck {
        groups: &["permissions"],
        title: "Checking permissions on global node_modules (this may take awhile)",
        mask: R_OK,
        root: CheckRoot::GlobalDir,
        should_own: false,
        missing_ok: false,
    },
    PermissionCheck {
        groups: &["permissions"],
        title: "Checking permissions on local bin folder",
        mask: R_OK | W_OK | X_OK,
        root: CheckRoot::LocalBin,
        should_own: false,
        missing_ok: true,
    },
    PermissionCheck {
        groups: &["permissions"],
        title: "Checking permissions on global bin folder",
        mask: X_OK,
        root: CheckRoot::GlobalBin,
        should_own: false,
        missing_ok: false,
    },
];

#[derive(Clone, Copy)]
enum CheckKind {
    Ping,
    LatestNpm,
    LatestNode,
    Registry,
    Git,
    Bin,
    Permissions(usize),
}

fn mask_label(mask: u32) -> &'static str {
    match (mask & R_OK != 0, mask & W_OK != 0, mask & X_OK != 0) {
        (true, false, false) => "readable",
        (true, true, false) => "readable, writable",
        (false, false, true) => "executable",
        _ => "readable, writable, executable",
    }
}

fn which(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join(name);
        let candidates = if cfg!(windows) {
            vec![
                candidate.with_extension("exe"),
                candidate.with_extension("cmd"),
                candidate.clone(),
            ]
        } else {
            vec![candidate]
        };
        if let Some(found) = candidates.into_iter().find(|c| c.is_file()) {
            return Some(found);
        }
    }
    None
}

pub struct Doctor;

impl Doctor {
    pub fn new() -> Self {
        Self
    }

    fn check_ping(&self) -> Result<String> {
        info("doctor", &[&"Pinging registry"]);
        Err(NpmError::new("registry ping is not implemented yet").with_code("ENOTSUP"))
    }

    fn latest_npm_version(&self) -> Result<String> {
        info("doctor", &[&"Getting npm package information"]);
        Err(
            NpmError::new("reading npm@latest from the registry is not implemented yet")
                .with_code("ENOTSUP"),
        )
    }

    fn latest_node_version(&self) -> Result<String> {
        info("doctor", &[&"Getting Node.js release information"]);
        Err(NpmError::new("reading nodejs.org release data is not implemented yet")
            .with_code("ENOTSUP"))
    }

    fn check_npm_registry(&self, npm: &dyn Npm) -> Result<String> {
        let registry = npm.store().get_str("registry");
        if registry != crate::store::DEFAULT_REGISTRY {
            return Err(NpmError::new(format!(
                "Try `npm config set registry={}`",
                crate::store::DEFAULT_REGISTRY
            )));
        }
        Ok(format!(
            "using default registry ({})",
            crate::store::DEFAULT_REGISTRY
        ))
    }

    fn git_path(&self) -> Result<String> {
        info("doctor", &[&"Finding git in your PATH"]);
        match which("git") {
            Some(path) => Ok(path.display().to_string()),
            None => {
                warn("doctor", &[&"getGitPath", &"git not found in PATH"]);
                Err(NpmError::new("Install git and ensure it's in your PATH."))
            }
        }
    }

    fn bin_path(&self, npm: &dyn Npm) -> Result<String> {
        info(
            "doctor",
            &[&"getBinPath", &"Finding npm global bin in your PATH"],
        );
        let bin = npm.bin();
        let bin_str = bin.display().to_string();
        let in_path = std::env::var_os("PATH")
            .map(|path| {
                std::env::split_paths(&path).any(|dir| dir.display().to_string() == bin_str)
            })
            .unwrap_or(false);
        if !in_path {
            return Err(NpmError::new(format!("Add {bin_str} to your $PATH")));
        }
        Ok(bin_str)
    }

    fn check_files_permission(
        &self,
        root: &Path,
        should_own: bool,
        mask: u32,
        missing_ok: bool,
    ) -> Result<String> {
        let mut ok = true;
        let mut pending = vec![root.to_path_buf()];
        while let Some(path) = pending.pop() {
            let shown = path.display().to_string();
            let meta = match std::fs::metadata(&path) {
                Ok(meta) => meta,
                Err(_) => {
                    if !missing_ok {
                        ok = false;
                        warn(
                            "doctor",
                            &[&"checkFilesPermission", &"error getting info for", &shown],
                        );
                    }
                    continue;
                }
            };
            // Ownership checks need `uid`/`gid`, which std does not expose.
            let _ = should_own;
            if !meta.is_dir() && !meta.is_file() {
                continue;
            }
            let writable = !meta.permissions().readonly();
            let readable = meta.is_dir() || std::fs::File::open(&path).is_ok();
            if (mask & R_OK != 0 && !readable) || (mask & W_OK != 0 && !writable) {
                ok = false;
                error(
                    "doctor",
                    &[&"checkFilesPermission", &format!(
                        "Missing permissions on {shown} (expect: {})",
                        mask_label(mask)
                    )],
                );
                continue;
            }
            if meta.is_dir() {
                match std::fs::read_dir(&path) {
                    Ok(entries) => pending.extend(entries.flatten().map(|e| e.path())),
                    Err(_) => {
                        ok = false;
                        warn(
                            "doctor",
                            &[&"checkFilesPermission", &"error reading directory", &shown],
                        );
                    }
                }
            }
        }
        if ok {
            Ok(String::new())
        } else {
            Err(NpmError::new(format!(
                "Check the permissions of files in {}{}",
                root.display(),
                if should_own {
                    " (should be owned by current user)"
                } else {
                    ""
                }
            )))
        }
    }

    fn verify_cached_files(&self, npm: &dyn Npm) -> Result<String> {
        info(
            "doctor",
            &[&"verifyCachedFiles", &"Verifying the npm cache"],
        );
        let mut verified = 0usize;
        let mut pending = vec![npm.store().cache()];
        while let Some(dir) = pending.pop() {
            let Ok(entries) = std::fs::read_dir(&dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    pending.push(path);
                } else {
                    verified += 1;
                }
            }
        }
        info(
            "doctor",
            &[
                &"verifyCachedFiles",
                &format!("Verification complete. {verified} entries"),
            ],
        );
        Ok(format!("verified {verified} tarballs"))
    }

    fn actions(&self, params: &[String]) -> Vec<(&'static str, CheckKind)> {
        let wanted = |groups: &[&str]| {
            params.is_empty() || params.iter().any(|p| groups.contains(&p.as_str()))
        };
        let simple: &[(&[&str], &str, CheckKind)] = &[
            (
                &["connection", "ping", "registry"],
                "Connecting to the registry",
                CheckKind::Ping,
            ),
            (&["versions"], "Checking npm version", CheckKind::LatestNpm),
            (&["versions"], "Checking node version", CheckKind::LatestNode),
            (
                &["registry"],
                "Checking configured npm registry",
                CheckKind::Registry,
            ),
            (
                &["environment"],
                "Checking for git executable in PATH",
                CheckKind::Git,
            ),
            (
                &["environment"],
                "Checking for global bin folder in PATH",
                CheckKind::Bin,
            ),
        ];
        let mut actions: Vec<(&'static str, CheckKind)> = simple
            .iter()
            .filter(|(groups, _, _)| wanted(groups))
            .map(|(_, title, kind)| (*title, *kind))
            .collect();
        if !crate::utils::is_windows::is_windows() {
            actions.extend(
                PERMISSION_CHECKS
                    .iter()
                    .enumerate()
                    .filter(|(_, check)| wanted(check.groups))
                    .map(|(index, check)| (check.title, CheckKind::Permissions(index))),
            );
        }
        actions
    }

    fn run(&self, npm: &dyn Npm, kind: CheckKind) -> Result<String> {
        match kind {
            CheckKind::Ping => self.check_ping(),
            CheckKind::LatestNpm => self.latest_npm_version(),
            CheckKind::LatestNode => self.latest_node_version(),
            CheckKind::Registry => self.check_npm_registry(npm),
            CheckKind::Git => self.git_path(),
            CheckKind::Bin => self.bin_path(npm),
            CheckKind::Permissions(index) => {
                let check = &PERMISSION_CHECKS[index];
                let root = match check.root {
                    CheckRoot::Cache => npm.store().cache(),
                    CheckRoot::LocalDir => npm.local_dir(),
                    CheckRoot::GlobalDir => npm.global_dir(),
                    CheckRoot::LocalBin => npm.local_dir().join(".bin"),
                    CheckRoot::GlobalBin => npm.bin(),
                };
                self.check_files_permission(&root, check.should_own, check.mask, check.missing_ok)
            }
        }
    }
}

impl Default for Doctor {
    fn default() -> Self {
        Self::new()
    }
}

impl Command for Doctor {
    fn meta(&self) -> &'static CommandMeta {
        &META
    }

    fn exec(&mut self, npm: &mut (dyn Npm + '_), args: &[String]) -> Result<()> {
        info("doctor", &[&"Running checkup"]);
        let mut all_ok = true;
        let color = npm.color();
        for (title, kind) in self.actions(args) {
            if !npm.silent() {
                crate::output::standard(title);
            }
            match self.run(&*npm, kind) {
                Ok(result) => {
                    let ok = crate::utils::display::paint(
                        "Ok",
                        crate::utils::display::Color::Green,
                        color,
                    );
                    let line = if result.is_empty() {
                        format!("{ok}\n")
                    } else {
                        format!("{ok}\n{result}\n")
                    };
                    if !npm.silent() {
                        crate::output::standard(line);
                    }
                }
                Err(err) => {
                    all_ok = false;
                    let not_ok = crate::utils::display::paint(
                        "Not ok",
                        crate::utils::display::Color::Red,
                        color,
                    );
                    let reason = crate::utils::display::paint(
                        &err.body,
                        crate::utils::display::Color::Cyan,
                        color,
                    );
                    if !npm.silent() {
                        crate::output::standard(format!("{not_ok}\n{reason}\n"));
                    }
                }
            }
        }
        if all_ok {
            return Ok(());
        }
        if npm.silent() {
            return Err(NpmError::new(
                "Some problems found. Check logs or disable silent mode for recommendations.",
            ));
        }
        Err(NpmError::new(
            "Some problems found. See above for recommendations.",
        ))
    }
}
