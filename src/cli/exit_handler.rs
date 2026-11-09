//! Port of `lib/cli/exit-handler.js`. Owns the "did we exit through the
//! normal path?" bookkeeping so early crashes still print a useful message.

use crate::npm::{Npm, NpmCli};

pub struct ExitHandler {
    npm: Option<NpmCli>,
    exited: bool,
    exit_error_message: bool,
    no_npm_error: bool,
}

impl Default for ExitHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl ExitHandler {
    pub fn new() -> Self {
        Self {
            npm: None,
            exited: false,
            exit_error_message: false,
            no_npm_error: false,
        }
    }

    pub fn set_npm(&mut self, npm: NpmCli) {
        self.npm = Some(npm);
    }

    pub fn npm_mut(&mut self) -> Option<&mut NpmCli> {
        self.npm.as_mut()
    }

    fn loaded(&self) -> bool {
        self.npm.as_ref().map(Npm::loaded).unwrap_or(false)
    }

    fn show_exit_error_message(&self) -> bool {
        if !self.loaded() {
            return false;
        }
        if !self.exited {
            return true;
        }
        self.exit_error_message
    }

    /// Ends the process with the state gathered so far.
    pub fn exit(&mut self, err: Option<&crate::utils::errors::NpmError>) -> i32 {
        self.exited = true;
        if !self.loaded() {
            self.log_console_error("Exit prior to config file resolving");
            return err.and_then(|e| e.exit_code).unwrap_or(1);
        }
        self.exit_error_message = err.map(|e| !e.suppress_error).unwrap_or(false);
        let code = err
            .and_then(|e| e.exit_code)
            .unwrap_or(if err.is_some() { 1 } else { 0 });
        if self.show_exit_error_message() {
            if let Some(npm) = &self.npm {
                crate::utils::log::error("", &[&npm.exit_error_message()]);
            }
        }
        code
    }

    fn log_console_error(&mut self, message: &str) {
        if self.npm.is_none() && !self.no_npm_error {
            self.no_npm_error = true;
            eprintln!("Exit prior to setting npm in exit handler: {message}");
            return;
        }
        eprintln!("{message}");
    }
}
