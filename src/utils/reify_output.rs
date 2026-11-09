//! Port of `lib/utils/reify-output.js`: reports what a reify changed.

use serde_json::Value;

use crate::npm::Npm;
use crate::utils::ms::ms;

#[derive(Debug, Default, Clone)]
pub struct ChangeItem {
    pub name: String,
    pub version: String,
    pub path: String,
}

#[derive(Debug, Clone)]
pub struct ChangeTo {
    pub from: ChangeItem,
    pub to: ChangeItem,
}

#[derive(Debug, Default, Clone)]
pub struct ReifySummary {
    pub added: usize,
    pub audited: usize,
    pub changed: usize,
    pub funding: usize,
    pub removed: usize,
    pub add: Vec<ChangeItem>,
    pub remove: Vec<ChangeItem>,
    pub change: Vec<ChangeTo>,
    pub unreviewed_scripts: Vec<Value>,
}

impl ReifySummary {
    pub fn to_json(&self, audit: Option<&Value>) -> Value {
        fn item(i: &ChangeItem) -> Value {
            serde_json::json!({ "name": i.name, "version": i.version, "path": i.path })
        }
        let mut map = serde_json::Map::new();
        map.insert(
            "add".into(),
            Value::Array(self.add.iter().map(item).collect()),
        );
        map.insert("added".into(), self.added.into());
        if let Some(audit) = audit {
            map.insert("audit".into(), audit.clone());
        }
        map.insert("audited".into(), self.audited.into());
        map.insert(
            "change".into(),
            Value::Array(
                self.change
                    .iter()
                    .map(|c| serde_json::json!({ "from": item(&c.from), "to": item(&c.to) }))
                    .collect(),
            ),
        );
        map.insert("changed".into(), self.changed.into());
        map.insert("funding".into(), self.funding.into());
        map.insert(
            "remove".into(),
            Value::Array(self.remove.iter().map(item).collect()),
        );
        map.insert("removed".into(), self.removed.into());
        if !self.unreviewed_scripts.is_empty() {
            map.insert(
                "unreviewedScripts".into(),
                Value::Array(self.unreviewed_scripts.clone()),
            );
        }
        Value::Object(map)
    }
}

fn plural(count: usize) -> &'static str {
    if count == 1 { "" } else { "s" }
}

/// Prints the `added N packages ... in 1.234s` line.
pub fn packages_changed_message(
    npm: &dyn Npm,
    summary: &ReifySummary,
    elapsed: std::time::Duration,
) {
    let ReifySummary {
        added,
        removed,
        changed,
        audited,
        ..
    } = *summary;
    let mut msg = String::from("\n");
    if added == 0 && removed == 0 && changed == 0 {
        msg.push_str("up to date");
        if audited > 0 {
            msg.push_str(", ");
        }
    } else {
        if added > 0 {
            msg.push_str(&format!("added {added} package{}", plural(added)));
        }
        if removed > 0 {
            if added > 0 {
                msg.push_str(", ");
            }
            if added > 0 && audited == 0 && changed == 0 {
                msg.push_str("and ");
            }
            msg.push_str(&format!("removed {removed} package{}", plural(removed)));
        }
        if changed > 0 {
            if added > 0 || removed > 0 {
                msg.push_str(", ");
            }
            if audited == 0 && (added > 0 || removed > 0) {
                msg.push_str("and ");
            }
            msg.push_str(&format!("changed {changed} package{}", plural(changed)));
        }
        if audited > 0 {
            msg.push_str(", and ");
        }
    }
    if audited > 0 {
        msg.push_str(&format!("audited {audited} package{}", plural(audited)));
    }
    msg.push_str(&format!(" in {}", ms(elapsed)));
    crate::output::set_meta(false);
    crate::output::standard(msg);
    let _ = npm;
}

/// Prints `N packages are looking for funding`.
pub fn packages_funding_message(npm: &dyn Npm, summary: &ReifySummary) {
    if summary.funding == 0 || npm.global() {
        return;
    }
    let pkg = if summary.funding == 1 {
        "package"
    } else {
        "packages"
    };
    let is = if summary.funding == 1 { "is" } else { "are" };
    crate::output::standard("");
    crate::output::standard(format!(
        "{} {pkg} {is} looking for funding",
        summary.funding
    ));
    crate::output::standard("  run `npm fund` for details");
}

/// Warns about install scripts that were blocked by `allowScripts`.
pub fn unreviewed_scripts_message(npm: &dyn Npm, scripts: &[Value]) {
    if scripts.is_empty() {
        return;
    }
    let count = scripts.len();
    let pkg = if count == 1 {
        "package had"
    } else {
        "packages had"
    };
    let header = format!(
        "{count} {pkg} install scripts blocked because they are not covered by allowScripts:"
    );

    let mut names = Vec::new();
    let mut lines = Vec::new();
    for script in scripts {
        let name = script
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("<unknown>");
        let version = script.get("version").and_then(Value::as_str);
        names.push(name.to_string());
        let ver = version.map(|v| format!("@{v}")).unwrap_or_default();
        let events = script
            .get("scripts")
            .and_then(Value::as_object)
            .map(|s| {
                s.iter()
                    .map(|(event, cmd)| format!("{event}: {}", cmd.as_str().unwrap_or_default()))
                    .collect::<Vec<_>>()
                    .join("; ")
            })
            .unwrap_or_default();
        lines.push(format!("  {name}{ver} ({events})"));
    }

    let remediation = if npm.global() {
        let list = names.join(",");
        vec![format!(
            "Run `npm install -g --allow-scripts={list}` to allow these scripts once, or `npm config set allow-scripts={list}` to allow them for all global installs."
        )]
    } else {
        vec![
            "Run `npm install-scripts ls` to review, or `npm install-scripts approve <pkg>` to allow."
                .to_string(),
        ]
    };

    let mut body = vec![header];
    body.extend(lines);
    body.push(String::new());
    body.extend(remediation);
    crate::utils::log::warn("install-scripts", &[&body.join("\n")]);
}

/// Entry point of `reify-output.js`.
pub fn reify_output(npm: &dyn Npm, summary: &ReifySummary, elapsed: std::time::Duration) {
    if npm.silent() {
        return;
    }
    if npm.json() {
        crate::output::set_meta(true);
        let value = serde_json::to_string_pretty(&summary.to_json(None)).unwrap_or_default();
        crate::output::standard(value);
        return;
    }
    packages_changed_message(npm, summary, elapsed);
    packages_funding_message(npm, summary);
    unreviewed_scripts_message(npm, &summary.unreviewed_scripts);
}
