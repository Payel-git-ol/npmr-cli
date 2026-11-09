//! Port of `lib/lifecycle-cmd.js`: commands that are just `npm run <script>`.

use crate::commands::command::{Command, CommandMeta};
use crate::npm::Npm;
use crate::utils::errors::Result;

pub trait LifecycleCommand: Command {
    fn script_event(&self) -> &'static str {
        self.meta().name
    }

    fn exec_lifecycle(&mut self, npm: &mut (dyn Npm + '_), args: &[String]) -> Result<()> {
        let mut run_args = vec![self.script_event().to_string()];
        run_args.extend(args.iter().cloned());
        npm.exec("run", &run_args)
    }
}

/// Builds the shared meta for restart/start/stop/test.
pub const fn lifecycle_meta(name: &'static str, description: &'static str) -> CommandMeta {
    CommandMeta::new(name, description)
        .with_usage(&["[-- <args>]"])
        .with_params(&["ignore-scripts", "script-shell"])
        .shellout()
        .workspaces()
}
