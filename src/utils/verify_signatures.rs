//! Port of `lib/utils/verify-signatures.js`: audits registry signatures and
//! attestations. Fetching manifests and signing keys needs `pacote` and TUF;
//! the audit bookkeeping, filtering and report are ported.

use std::collections::BTreeSet;

use serde_json::{json, Value};

use crate::npm::Npm;
use crate::utils::display::{paint, Color};
use crate::utils::errors::{NpmError, Result};
use crate::utils::log::verbose;

/// A dependency edge from the install tree, reduced to what the audit reads.
#[derive(Debug, Clone, Default)]
pub struct AuditEdge {
    pub name: String,
    pub spec: String,
    /// Installed location of the target, when resolved.
    pub to_location: Option<String>,
    /// The depending node's location, used by `--workspace` filtering.
    pub from_location: Option<String>,
    pub to_version: Option<String>,
    pub optional: bool,
    pub peer: bool,
    pub dev: bool,
    pub workspace_target: bool,
    /// `"MISSING"` when arborist could not place the dependency.
    pub error: Option<String>,
}

impl AuditEdge {
    fn edge_type(&self) -> &'static str {
        if self.optional {
            "optionalDependencies"
        } else if self.peer {
            "peerDependencies"
        } else if self.dev {
            "devDependencies"
        } else {
            "dependencies"
        }
    }

    /// The registry spec `name@spec`, unless it is not a registry spec.
    pub fn registry_spec(&self) -> Option<(String, String)> {
        if !is_registry_spec(&self.spec) {
            return None;
        }
        Some((self.name.clone(), self.spec.trim_start_matches("registry:").to_string()))
    }
}

fn is_registry_spec(spec: &str) -> bool {
    !(spec.starts_with("file:")
        || spec.starts_with("git")
        || spec.starts_with("http:")
        || spec.starts_with("https:")
        || spec.starts_with("github:")
        || spec.starts_with("gitlab:")
        || spec.starts_with("bitbucket:")
        || spec.starts_with("link:"))
}

/// A package whose signature or attestation did not verify.
#[derive(Debug, Clone, Default)]
pub struct AuditFinding {
    pub code: Option<String>,
    pub message: Option<String>,
    pub integrity: Option<String>,
    pub keyid: Option<String>,
    pub location: String,
    pub name: String,
    pub registry: String,
    pub resolved: Option<String>,
    pub signature: Option<String>,
    pub predicate_type: Option<String>,
    pub r#type: Option<String>,
    pub version: String,
}

impl AuditFinding {
    fn key(&self) -> String {
        format!("{}@{}", self.name, self.version)
    }
}

#[derive(Debug, Clone, Default)]
pub struct VerifiedAttestation {
    name: String,
    version: String,
    location: String,
    registry: String,
}

/// Result of a completed audit.
#[derive(Debug, Default)]
pub struct AuditResult {
    pub invalid: Vec<AuditFinding>,
    pub missing: Vec<AuditFinding>,
    pub verified: Vec<VerifiedAttestation>,
    pub audited_with_keys: usize,
    pub verified_signatures: usize,
    pub verified_attestations: usize,
    pub has_registry_keys: bool,
}

/// What a signature check produced for one package.
#[derive(Debug, Clone, Default)]
pub struct SignatureCheck {
    pub signatures: usize,
    pub attestations: bool,
    pub integrity: Option<String>,
    pub resolved: Option<String>,
    pub invalid: Option<AuditFinding>,
}

struct Candidate {
    name: String,
    version: String,
    location: String,
    registry: String,
    #[expect(dead_code, reason = "reported in the JSON findings")]
    r#type: String,
}

pub struct VerifySignatures<'a> {
    npm: &'a dyn Npm,
    edges: Vec<AuditEdge>,
    filters: BTreeSet<String>,
    keys_registries: BTreeSet<String>,
}

impl<'a> VerifySignatures<'a> {
    pub fn new(npm: &'a dyn Npm, edges: Vec<AuditEdge>, filters: BTreeSet<String>) -> Self {
        Self {
            npm,
            edges,
            filters,
            keys_registries: BTreeSet::new(),
        }
    }

    /// Edges that survive `--workspace` filtering, keyed by target location.
    fn selected_edges(&mut self) -> Result<Vec<AuditEdge>> {
        let mut out = Vec::new();
        for edge in &self.edges {
            let filtered_out = !self.filters.is_empty()
                && edge
                    .from_location
                    .as_ref()
                    .is_some_and(|from| !self.filters.contains(from));
            if filtered_out {
                continue;
            }
            if let Some((_, _)) = edge.registry_spec() {
                self.keys_registries.insert("registry".to_string());
            }
            out.push(edge.clone());
        }
        if out.is_empty() {
            return Err(NpmError::new("found no installed dependencies to audit"));
        }
        Ok(out)
    }

    fn candidate(&self, edge: &AuditEdge) -> Option<Candidate> {
        let r#type = edge.edge_type().to_string();
        // Packages that may be omitted during install are not required.
        if edge.error.as_deref() == Some("MISSING") && r#type != "dependencies" {
            return None;
        }
        let (name, _spec) = edge.registry_spec()?;
        let version = edge.to_version.clone().filter(|v| !v.is_empty())?;
        if edge.workspace_target {
            return None;
        }
        for omit in &["dev", "optional", "peer"] {
            if self.npm.store().get_bool(omit) {
                let flag = match *omit {
                    "dev" => edge.dev,
                    "optional" => edge.optional,
                    _ => edge.peer,
                };
                if flag {
                    return None;
                }
            }
        }
        Some(Candidate {
            name,
            version,
            location: edge.to_location.clone().unwrap_or_default(),
            registry: "registry".to_string(),
            r#type,
        })
    }

    /// Audits every selected edge. `check` performs the signature and
    /// attestation verification for one `name@version` on a registry;
    /// `has_keys` says whether that registry publishes signing keys.
    pub fn run(
        &mut self,
        has_keys: impl Fn(&str) -> bool,
        check: impl Fn(&str, &str, &str) -> Result<SignatureCheck>,
    ) -> Result<AuditResult> {
        let edges = self.selected_edges()?;
        let mut result = AuditResult::default();
        let mut checked = BTreeSet::new();

        for edge in &edges {
            let Some(candidate) = self.candidate(edge) else {
                continue;
            };
            if !checked.insert(candidate.location.clone()) {
                continue;
            }
            let keys = has_keys(&candidate.registry);
            result.has_registry_keys |= keys;
            if keys {
                result.audited_with_keys += 1;
            }

            let check = check(&candidate.name, &candidate.version, &candidate.registry)?;
            if let Some(invalid) = check.invalid {
                result.invalid.push(invalid);
                continue;
            }
            if check.signatures > 0 {
                result.verified_signatures += 1;
            } else if keys {
                result.missing.push(AuditFinding {
                    integrity: check.integrity,
                    location: candidate.location.clone(),
                    name: candidate.name.clone(),
                    registry: candidate.registry.clone(),
                    resolved: check.resolved,
                    version: candidate.version.clone(),
                    ..Default::default()
                });
            }
            if check.attestations {
                result.verified_attestations += 1;
                if self.npm.store().get_bool("include-attestations") {
                    result.verified.push(VerifiedAttestation {
                        name: candidate.name,
                        version: candidate.version,
                        location: candidate.location,
                        registry: candidate.registry,
                    });
                }
            }
        }

        if result.audited_with_keys == 0 && result.verified_attestations == 0 {
            return Err(NpmError::new(
                "found no dependencies to audit that were installed from a supported registry",
            ));
        }
        result.invalid.sort_by_key(|a| a.key());
        result.missing.sort_by_key(|a| a.key());
        Ok(result)
    }

    /// Prints the report and returns whether anything failed verification.
    pub fn report(&self, result: &AuditResult, elapsed_secs: u64) -> bool {
        use crate::output::standard;

        let json = self.npm.store().get_bool("json");
        let include_attestations = self.npm.store().get_bool("include-attestations");
        let has_problems = !result.invalid.is_empty() || !result.missing.is_empty();
        if has_problems {
            crate::output::set_exit_code(1);
        }
        if json {
            let mut payload = json!({
                "invalid": finding_list(&result.invalid),
                "missing": finding_list(&result.missing),
            });
            if include_attestations {
                payload["verified"] = Value::Array(
                    result
                        .verified
                        .iter()
                        .map(|v| {
                            json!({
                                "name": v.name,
                                "version": v.version,
                                "location": v.location,
                                "registry": v.registry,
                            })
                        })
                        .collect(),
                );
            }
            standard(format!("{}\n", serde_json::to_string_pretty(&payload).unwrap_or_default()));
            return has_problems;
        }

        verbose("verifying registry signatures", &[]);
        let color = self.npm.color();
        let plurals = |n: usize, one: &str, many: &str| {
            if n == 1 {
                one.to_string()
            } else {
                many.to_string()
            }
        };
        let audited_plural = if result.audited_with_keys > 1 { "s" } else { "" };
        standard(format!(
            "audited {} package{audited_plural} in {elapsed_secs}s\n",
            result.audited_with_keys
        ));

        let verified = paint("verified", Color::White, color);
        if result.verified_signatures > 0 {
            standard(format!(
                "{}\n",
                if result.verified_signatures == 1 {
                    format!("1 package has a {verified} registry signature")
                } else {
                    format!(
                        "{} packages have {verified} registry signatures",
                        result.verified_signatures
                    )
                }
            ));
        }
        if result.verified_attestations > 0 {
            standard(format!(
                "{}\n",
                if result.verified_attestations == 1 {
                    format!("1 package has a {verified} attestation")
                } else {
                    format!(
                        "{} packages have {verified} attestations",
                        result.verified_attestations
                    )
                }
            ));
            if !include_attestations {
                standard("(use --json --include-attestations to view attestation details)\n");
            }
        }

        if !result.missing.is_empty() {
            let missing = paint("missing", Color::Red, color);
            standard(plurals(
                result.missing.len(),
                &format!(
                    "1 package has a {missing} registry signature but the registry is providing signing keys:\n"
                ),
                &format!(
                    "{} packages have {missing} registry signatures but the registry is providing signing keys:\n",
                    result.missing.len()
                ),
            ));
            for entry in &result.missing {
                standard(format!(
                    "{} ({})\n",
                    paint(&entry.key(), Color::Red, color),
                    entry.registry
                ));
            }
        }

        if !result.invalid.is_empty() {
            if !result.missing.is_empty() {
                standard("\n");
            }
            let invalid = paint("invalid", Color::Red, color);
            let signatures: Vec<&AuditFinding> = result
                .invalid
                .iter()
                .filter(|i| i.code.as_deref() == Some("EINTEGRITYSIGNATURE"))
                .collect();
            if !signatures.is_empty() {
                standard(plurals(
                    signatures.len(),
                    &format!("1 package has an {invalid} registry signature:\n"),
                    &format!(
                        "{} packages have {invalid} registry signatures:\n",
                        signatures.len()
                    ),
                ));
                for entry in &signatures {
                    standard(format!(
                        "{} ({})\n",
                        paint(&entry.key(), Color::Red, color),
                        entry.registry
                    ));
                }
                standard("\n");
            }
            let attestations: Vec<&AuditFinding> = result
                .invalid
                .iter()
                .filter(|i| i.code.as_deref() == Some("EATTESTATIONVERIFY"))
                .collect();
            if !attestations.is_empty() {
                standard(plurals(
                    attestations.len(),
                    &format!("1 package has an {invalid} attestation:\n"),
                    &format!(
                        "{} packages have {invalid} attestations:\n",
                        attestations.len()
                    ),
                ));
                for entry in &attestations {
                    standard(format!(
                        "{} ({})\n",
                        paint(&entry.key(), Color::Red, color),
                        entry.registry
                    ));
                }
                standard("\n");
            }
            standard(format!(
                "{}\n",
                if result.invalid.len() == 1 {
                    "Someone might have tampered with this package since it was published on the registry!"
                } else {
                    "Someone might have tampered with these packages since they were published on the registry!"
                }
            ));
        }
        has_problems
    }
}

fn finding_list(findings: &[AuditFinding]) -> Value {
    Value::Array(
        findings
            .iter()
            .map(|f| {
                json!({
                    "code": f.code,
                    "message": f.message,
                    "integrity": f.integrity,
                    "keyid": f.keyid,
                    "location": f.location,
                    "name": f.name,
                    "registry": f.registry,
                    "resolved": f.resolved,
                    "signature": f.signature,
                    "predicateType": f.predicate_type,
                    "type": f.r#type,
                    "version": f.version,
                })
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_specs() {
        let prod = AuditEdge {
            name: "a".into(),
            spec: "^1".into(),
            ..Default::default()
        };
        assert_eq!(prod.registry_spec(), Some(("a".into(), "^1".into())));
        assert_eq!(prod.edge_type(), "dependencies");
        let git = AuditEdge {
            spec: "git+https://x".into(),
            ..Default::default()
        };
        assert_eq!(git.registry_spec(), None);
        let file = AuditEdge {
            spec: "file:../x".into(),
            ..Default::default()
        };
        assert_eq!(file.registry_spec(), None);
    }

    #[test]
    fn edge_types_prefer_optional_then_peer() {
        let edge = AuditEdge {
            optional: true,
            peer: true,
            dev: true,
            ..Default::default()
        };
        assert_eq!(edge.edge_type(), "optionalDependencies");
    }
}
