//! Port of `lib/cli.js` and `lib/cli/entry.js`, collapsed into one entry
//! point: boot, load config, run one command, report errors, set exit code.

pub mod exit_handler;
pub mod update_notifier;
pub mod validate_engines;

use crate::npm::{Npm, NpmCli};
use crate::utils::errors::NpmError;
use crate::utils::log::Level;

/// Process entry point used by `main.rs`.
pub fn main() {
    // Keep the original argv for config parsing, then drop the executable
    // path: `Store` expects argv[0] to be the program name.
    let argv: Vec<String> = std::env::args().collect();
    let mut npm = NpmCli::new(argv);
    let code = run(&mut npm);
    std::process::exit(code);
}

fn run(npm: &mut NpmCli) -> i32 {
    crate::utils::log::set_level(Level::Notice);
    crate::utils::log::verbose("cli", &[&current_exe_display()]);

    match npm.load() {
        Ok(load) => {
            if !load.exec {
                npm.finish();
                return 0;
            }
            let Some(command) = load.command else {
                println!("{}", npm.usage());
                npm.finish();
                return 1;
            };

            let non_dash: Vec<&String> = load
                .args
                .iter()
                .filter(|a| a.chars().next().is_some_and(is_unicode_dash))
                .collect();
            if !non_dash.is_empty() {
                let joined = non_dash
                    .iter()
                    .map(|s| s.as_str())
                    .collect::<Vec<_>>()
                    .join(", ");
                crate::utils::log::error(
                    "arg",
                    &[
                        &"Argument starts with non-ascii dash, this is probably invalid:",
                        &crate::utils::redact::redact(&joined),
                    ],
                );
            }

            match npm.run(&command, &load.args) {
                Ok(()) => {
                    npm.finish();
                    0
                }
                Err(mut err) => finish_with_error(npm, &mut err),
            }
        }
        Err(mut err) => finish_with_error(npm, &mut err),
    }
}

fn finish_with_error(npm: &mut NpmCli, err: &mut NpmError) -> i32 {
    npm.handle_error(err);
    npm.finish();

    if err.suppress_error {
        return err.exit_code.unwrap_or(1);
    }

    // `--json` merges the error into the machine readable output.
    if npm.store().get_bool("json") {
        let payload = serde_json::json!({
            "error": {
                "code": err.code,
                "summary": err.summary.join("\n").trim(),
                "detail": err.detail.join("\n").trim(),
            }
        });
        println!("{}", serde_json::to_string_pretty(&payload).unwrap_or_default());
    }

    eprintln!("\x1b[31mnpm\x1b[0m \x1b[31merror\x1b[0m {}", err.banner("npm ERR!"));
    if !err.detail.is_empty() {
        for line in &err.detail {
            eprintln!("npm ERR! {line}");
        }
    }
    crate::utils::log::error("", &[&npm.exit_error_message()]);

    err.exit_code
        .or(if err.code.is_some() { Some(1) } else { None })
        .unwrap_or(1)
}

fn current_exe_display() -> String {
    std::env::args()
        .take(2)
        .collect::<Vec<_>>()
        .join(" ")
}

/// Hyphen-minus is the only valid dash prefix; these look alike but are not.
fn is_unicode_dash(c: char) -> bool {
    matches!(
        c as u32,
        0x2010..=0x2015 | 0x2212 | 0xFE58 | 0xFE63 | 0xFF0D
    )
}
