//! Port of `lib/utils/npm-usage.js`.

use crate::npm::Npm;
use crate::utils::cmd_list::COMMANDS;

const COL_MAX: usize = 60;
const COL_MIN: usize = 24;
const COL_GUTTER: usize = 16;
const INDENT: usize = 4;

fn indent(repeat: usize) -> String {
    " ".repeat(repeat)
}

fn indent_newline(repeat: usize) -> String {
    format!("\n{}", indent(repeat))
}

pub fn usage(npm: &(dyn Npm + '_)) -> String {
    let store = npm.store();
    let browser = if store.get_str("viewer") == "browser" {
        " (in a browser)"
    } else {
        ""
    };
    let all_commands = if store.get_bool("long") {
        cmd_usages(npm)
    } else {
        cmd_names()
    };

    format!(
        "npm <command>\n\
        \n\
        Usage:\n\
        \n\
        npm install        install all the dependencies in your project\n\
        npm install <foo>  add the <foo> dependency to your project\n\
        npm test           run this project's tests\n\
        npm run <foo>      run the script named <foo>\n\
        npm <command> -h   quick help on <command>\n\
        npm -l             display usage info for all commands\n\
        npm help <term>    search for help on <term>{browser}\n\
        npm help npm       more involved overview{browser}\n\
        \n\
        All commands:\n\
        {all_commands}\n\
        \n\
        Specify configs in the ini-formatted file:\n\
        {userconfig}\n\
        or on the command line via: npm <command> --key=value\n\
        \n\
        More configuration info: npm help config\n\
        Configuration fields: npm help 7 config\n\
        \n\
        npm@{version} {root}",
        userconfig = indent(INDENT) + &store.get_str("userconfig"),
        version = npm.version(),
        root = npm.npm_root().to_string_lossy(),
    )
}

fn cmd_names() -> String {
    let mut out: Vec<String> = vec![String::new()];

    let columns = terminal_columns();
    let line = if columns == 0 {
        COL_MAX
    } else {
        COL_MAX.min(columns.saturating_sub(COL_GUTTER).max(COL_MIN))
    };

    let mut l = 0usize;
    for c in COMMANDS {
        let current = &out[l];
        if current.len() + c.len() + 2 < line {
            out[l].push_str(", ");
            out[l].push_str(c);
        } else {
            out[l].push(',');
            l += 1;
            out.push((*c).to_string());
        }
    }

    let joined = out.join(&indent_newline(INDENT));
    format!("{}{}", indent_newline(INDENT), &joined[2.min(joined.len())..])
}

fn cmd_usages(npm: &(dyn Npm + '_)) -> String {
    let mut max_len = 0usize;
    let mut set: Vec<(&'static str, Vec<String>)> = Vec::new();
    for c in COMMANDS {
        let usage = crate::commands::usage_for(c, None, false)
            .split('\n')
            .map(str::to_string)
            .collect();
        max_len = max_len.max(c.len());
        set.push((c, usage));
    }
    let _ = npm;
    set.iter()
        .map(|(name, usage_lines)| {
            let gutter = indent(max_len - name.len() + 1);
            let usage = usage_lines.join(&indent_newline(INDENT + max_len + 1));
            format!("{}{}{}{}", indent_newline(INDENT), name, gutter, usage)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn terminal_columns() -> usize {
    std::env::var("COLUMNS")
        .ok()
        .and_then(|c| c.parse().ok())
        .unwrap_or(80)
}
