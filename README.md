# npmr - npm rewritten in Rust

An experimental fork of [npm](https://github.com/npm/cli) rewritten in Rust. The goal
is to move away from the slow JavaScript implementation to a faster native binary.
Every module under `lib/` was translated into a single Rust crate, and the JavaScript
sources have been removed. Commands share npm's configuration layers, argument parsing,
logging format and output, so the port behaves like the CLI it replaces wherever the
port is implemented.

The project is a work in progress: commands that depend on the npm registry, the
install-tree ("arborist") or tarball fetching report `ENOTSUP` instead of silently
doing nothing.

### Requirements

* Rust 1.85 or newer (edition 2024)

No Node.js is required to build or run the binary.

### Build

```bash
cargo build --release
./target/release/npmr --version
```

Dependencies are pinned in `Cargo.lock`; for an air-gapped build use
`cargo build --offline`.

### Usage

```bash
npmr <command> [--flag[=value]] [args...]
```

`npmr --help` lists every command, and `npmr <command> -h` gives a per-command
usage summary. Configuration is read from the same places as npm: built-in
defaults, the global `.npmrc`, the user `.npmrc`, the project `.npmrc` and the
command line, in that order of precedence.

### Project layout

| Path | Contents |
| --- | --- |
| `src/main.rs` | Binary entry point |
| `src/cli/` | Startup: config loading, command dispatch, exit handler, update notifier, engine checks |
| `src/npm.rs` | `Npm` trait shared by commands (config store, manifest, output, bins) |
| `src/store.rs` | Layered configuration store replacing `@npmcli/config` |
| `src/cache.rs` | In-memory (moka) cache for parsed `package.json` and `.npmrc` files, invalidated on mtime change |
| `src/commands/` | One module per npm command |
| `src/utils/` | Helpers ported from `lib/utils` (logging, errors, auth, workspaces, sbom, signatures, ...) |
| `src/completion/` | Shell completion scripts for bash, zsh, fish and PowerShell |

### Port status

Configuration, argument parsing, logging, error reporting, help, completion,
`config`, `pkg`, `init`, `run`/`exec` scripts, `allow-scripts` tooling, workspace
helpers, SBOM export, signature verification and the other self-contained commands
are ported.

Anything that needs network access or the install tree is stubbed with an
`ENOTSUP` error:

* registry access - `access`, `deprecate`, `dist-tag`, `org`, `owner`, `search`,
  `view`, `unpublish`, `token`, `trust`, `stage`;
* install tree (arborist) - `install`, `ci`, `update`, `ls`, `audit`, `outdated`,
  `dedupe`, `prune`, `explain`, `fund`, `diff`;
* package fetching - `pack`, `publish`, `patch`, `cache add`, `doctor` checks.

### Links & Resources

* [**Documentation**](https://docs.npmjs.com/) - Official docs & how-tos for all things **npm**
    * Note: `npmr help-search <query>` searches the local docs
* [**Upstream CLI**](https://github.com/npm/cli) - The JavaScript implementation this port follows
* [**RFCs**](https://github.com/npm/rfcs) - Design discussions for the npm CLI and registry
* [**Service Status**](https://status.npmjs.org/) - Status of the npm website & registry

### Acknowledgments

* `npmr` is configured to use the **npm Public Registry** at
  [https://registry.npmjs.org](https://registry.npmjs.org) by default; usage of this
  registry is subject to the **Terms of Use** available at
  [https://npmjs.com/policies/terms](https://npmjs.com/policies/terms)
* You can configure `npmr` to use any other compatible registry; see
  [configuring third-party registries](https://docs.npmjs.com/cli/v7/using-npm/registry)

### FAQ on Branding

#### Is it "npm" or "NPM" or "Npm"?

**`npm`** should never be capitalized unless it is being displayed in a location that is customarily all-capitals (ex. titles on `man` pages).

#### Is "npm" an acronym for "Node Package Manager"?

Contrary to popular belief, **`npm`** **is not** an acronym for "Node Package Manager." It is a recursive backronymic abbreviation for **"npm is not an acronym"** (if the project were named "ninaa," then it would be an acronym). The precursor to **`npm`** was actually a bash utility named **"pm"**, which was the shortform name of **"pkgmakeinst"** - a bash function that installed various things on various platforms. If **`npm`** were ever considered an acronym, it would be as "node pm" or, potentially, "new pm".
