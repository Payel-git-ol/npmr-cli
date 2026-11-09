//! Port of `lib/utils/sbom-cyclonedx.js`: renders the install tree as a
//! CycloneDX 1.5 BOM.

use serde_json::{json, Map, Value};

use crate::npm::Npm;

const CYCLONEDX_SCHEMA: &str = "http://cyclonedx.org/schema/bom-1.5.schema.json";
const CYCLONEDX_FORMAT: &str = "CycloneDX";
const CYCLONEDX_SCHEMA_VERSION: &str = "1.5";

const PROP_BUNDLED: &str = "cdx:npm:package:bundled";
const PROP_DEVELOPMENT: &str = "cdx:npm:package:development";
const PROP_EXTRANEOUS: &str = "cdx:npm:package:extraneous";
const PROP_PRIVATE: &str = "cdx:npm:package:private";

/// A dependency edge leaving a node, enough to build the BOM graph.
#[derive(Debug, Clone)]
pub struct SbomEdge {
    /// `prod`, `dev`, `optional`, `peer` or `workspace`.
    pub edge_type: String,
    pub to: usize,
}

/// One node of the installed tree, reduced to what the SBOM formats read.
#[derive(Debug, Clone, Default)]
pub struct SbomNode {
    pub name: String,
    pub version: String,
    pub is_root: bool,
    pub is_link: bool,
    /// The manifest at this location, if any.
    pub package: Option<Value>,
    /// `resolved` tarball URL or git spec.
    pub resolved: Option<String>,
    /// Subresource integrity string such as `sha512-...`.
    pub integrity: Option<String>,
    pub dev: bool,
    pub optional: bool,
    pub dev_optional: bool,
    pub extraneous: bool,
    pub bundled: bool,
    pub workspace: bool,
    pub location: Option<String>,
    pub edges_out: Vec<SbomEdge>,
}

impl SbomNode {
    pub fn pkgid(&self) -> String {
        format!("{}@{}", self.name, self.version)
    }
}

/// A random version-4 UUID, formatted like `crypto.randomUUID()`.
pub fn random_uuid() -> String {
    use sha2::{Digest, Sha256};

    let mut seed = Vec::new();
    seed.extend_from_slice(&std::process::id().to_le_bytes());
    seed.extend_from_slice(
        &std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or_default()
            .to_le_bytes(),
    );
    seed.extend_from_slice(&uuid_counter().to_le_bytes());
    let digest = Sha256::digest(&seed);
    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(&digest[..16]);
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let hex = hex::encode(bytes);
    format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    )
}

fn uuid_counter() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    COUNTER.fetch_add(1, Ordering::Relaxed)
}

fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

fn package_field<'a>(node: &'a SbomNode, key: &str) -> Option<&'a Value> {
    node.package.as_ref()?.get(key)
}

fn package_str<'a>(node: &'a SbomNode, key: &str) -> Option<&'a str> {
    package_field(node, key).and_then(Value::as_str)
}

/// Reads `license.type`, else joins `licenses` with ` OR `.
fn license(node: &SbomNode) -> Option<String> {
    if let Some(license) = package_field(node, "license") {
        if let Some(text) = license.as_str() {
            return Some(text.to_string());
        }
        if let Some(kind) = license.get("type").and_then(Value::as_str) {
            return Some(kind.to_string());
        }
    }
    let licenses = package_field(node, "licenses").and_then(Value::as_array)?;
    let joined: Vec<&str> = licenses
        .iter()
        .filter_map(|l| l.as_str().or_else(|| l.get("type").and_then(Value::as_str)))
        .filter(|l| !l.is_empty())
        .collect();
    (!joined.is_empty()).then(|| joined.join(" OR "))
}

fn is_git_node(node: &SbomNode) -> bool {
    let Some(resolved) = node.resolved.as_deref() else {
        return false;
    };
    resolved.starts_with("git+")
        || resolved.starts_with("git:")
        || resolved.starts_with("git@")
        || resolved.starts_with("github:")
        || resolved.starts_with("gitlab:")
        || resolved.starts_with("bitbucket:")
        || resolved.contains("://github.com/")
        || resolved.contains("://gitlab.com/")
        || resolved.contains("://bitbucket.org/")
}

fn purl(node: &SbomNode) -> String {
    let name = &node.name;
    let mut purl = if let Some(rest) = name.strip_prefix('@') {
        match rest.split_once('/') {
            Some((scope, package)) => format!(
                "pkg:npm/%40{}%2F{}@{}",
                percent_encode(scope),
                percent_encode(package),
                percent_encode(&node.version)
            ),
            None => format!("pkg:npm/{}@{}", percent_encode(rest), percent_encode(&node.version)),
        }
    } else {
        format!("pkg:npm/{}@{}", percent_encode(name), percent_encode(&node.version))
    };
    if is_git_node(node)
        && let Some(resolved) = &node.resolved
    {
        purl.push_str(&format!("?vcs_url={}", percent_encode(resolved)));
    }
    purl
}

const PURL_SET: &percent_encoding::AsciiSet = &percent_encoding::CONTROLS
    .add(b' ')
    .add(b'/')
    .add(b'%')
    .add(b'@');

fn percent_encode(text: &str) -> String {
    percent_encoding::utf8_percent_encode(text, PURL_SET).to_string()
}

/// Splits an SRI string into `(algorithm, hex digest)`.
fn integrity_digest(integrity: &str) -> Option<(&str, String)> {
    let first = integrity.split_whitespace().find(|s| !s.starts_with('?'))?;
    let (algorithm, digest) = first.split_once('-')?;
    let bytes = base64_decode(digest)?;
    Some((algorithm, hex::encode(bytes)))
}

fn base64_decode(text: &str) -> Option<Vec<u8>> {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = Vec::new();
    let mut buffer = 0u32;
    let mut bits = 0u32;
    for byte in text.bytes() {
        let value = match byte {
            b'=' => break,
            b => TABLE.iter().position(|&c| c == b)? as u32,
        };
        buffer = (buffer << 6) | value;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buffer >> bits) as u8);
        }
    }
    Some(out)
}

fn prop(name: &str) -> Value {
    json!({ "name": name, "value": "true" })
}

fn ext_ref(kind: &str, url: &str) -> Value {
    json!({ "type": kind, "url": url })
}

fn to_cyclonedx_id(node: &SbomNode) -> String {
    format!("{}@{}", node.name, node.version)
}

/// A node is described by a single `name@version` reference once nodes with
/// the same identity are deduplicated.
pub fn cyclonedx_item(node: &SbomNode, package_type: Option<&str>) -> Value {
    let package_type = package_type.unwrap_or("library");
    let scope = if node.optional || node.dev_optional {
        "optional"
    } else {
        "required"
    };
    let author = match package_field(node, "author") {
        Some(Value::Object(map)) => map.get("name").cloned(),
        Some(Value::String(text)) => Some(Value::String(text.clone())),
        _ => None,
    };
    let mut component = Map::new();
    component.insert("bom-ref".into(), json!(to_cyclonedx_id(node)));
    component.insert("type".into(), json!(package_type));
    component.insert("name".into(), json!(node.name));
    component.insert("version".into(), json!(node.version));
    component.insert("scope".into(), json!(scope));
    if let Some(author) = author {
        component.insert("author".into(), author);
    }
    if let Some(description) = package_str(node, "description") {
        component.insert("description".into(), json!(description));
    }
    component.insert("purl".into(), json!(purl(node)));
    let mut properties = Vec::new();
    if let Some(integrity) = &node.integrity {
        if let Some((algorithm, hex_digest)) = integrity_digest(integrity) {
            let alg = match algorithm {
                "sha1" => "SHA-1",
                "sha256" => "SHA-256",
                "sha384" => "SHA-384",
                "sha512" => "SHA-512",
                _ => "SHA-512",
            };
            component.insert(
                "hashes".into(),
                json!([{ "alg": alg, "content": hex_digest }]),
            );
        }
    }
    if node.dev {
        properties.push(prop(PROP_DEVELOPMENT));
    }
    if package_field(node, "private").and_then(Value::as_bool) == Some(true) {
        properties.push(prop(PROP_PRIVATE));
    }
    if node.extraneous {
        properties.push(prop(PROP_EXTRANEOUS));
    }
    if node.bundled {
        properties.push(prop(PROP_BUNDLED));
    }
    component.insert("properties".into(), Value::Array(properties));

    let mut references = Vec::new();
    if let Some(resolved) = &node.resolved
        && !node.is_link
    {
        references.push(ext_ref("distribution", resolved));
    }
    if let Some(url) = package_field(node, "repository")
        .and_then(|r| r.get("url"))
        .and_then(Value::as_str)
    {
        references.push(ext_ref("vcs", url));
    }
    if let Some(homepage) = package_str(node, "homepage") {
        references.push(ext_ref("website", homepage));
    }
    if let Some(url) = package_field(node, "bugs")
        .and_then(|b| b.get("url"))
        .and_then(Value::as_str)
    {
        references.push(ext_ref("issue-tracker", url));
    }
    component.insert("externalReferences".into(), Value::Array(references));

    if let Some(license) = license(node) {
        component.insert("licenses".into(), json!([{ "license": { "id": license } }]));
    }
    Value::Object(component)
}

fn cyclonedx_dependency(node: &SbomNode, nodes: &[SbomNode]) -> Value {
    let mut depends_on: Vec<String> = Vec::new();
    for edge in &node.edges_out {
        let Some(target) = nodes.get(edge.to) else {
            continue;
        };
        let id = to_cyclonedx_id(target);
        if !depends_on.contains(&id) {
            depends_on.push(id);
        }
    }
    json!({ "ref": to_cyclonedx_id(node), "dependsOn": depends_on })
}

/// Builds the BOM. Nodes are deduplicated by `name@version`, keeping the
/// first occurrence.
pub fn cyclonedx_output(
    npm: &dyn Npm,
    nodes: &[SbomNode],
    package_type: Option<&str>,
    package_lock_only: bool,
) -> Value {
    let root = nodes.iter().position(|node| node.is_root);
    let mut children: Vec<&SbomNode> = Vec::new();
    for node in nodes.iter().filter(|node| !node.is_root && !node.is_link) {
        let id = to_cyclonedx_id(node);
        if !children.iter().any(|seen| to_cyclonedx_id(seen) == id) {
            children.push(node);
        }
    }

    let mut dependencies = Vec::new();
    if let Some(root) = root.map(|index| &nodes[index]) {
        dependencies.push(cyclonedx_dependency(root, nodes));
    }
    dependencies.extend(
        children
            .iter()
            .map(|node| cyclonedx_dependency(node, nodes)),
    );

    let component = root
        .map(|index| cyclonedx_item(&nodes[index], package_type))
        .unwrap_or(Value::Null);
    let lifecycle = if package_lock_only {
        "pre-build"
    } else {
        "build"
    };
    json!({
        "$schema": CYCLONEDX_SCHEMA,
        "bomFormat": CYCLONEDX_FORMAT,
        "specVersion": CYCLONEDX_SCHEMA_VERSION,
        "serialNumber": format!("urn:uuid:{}", random_uuid()),
        "version": 1,
        "metadata": {
            "timestamp": now_iso(),
            "lifecycles": [{ "phase": lifecycle }],
            "tools": [{
                "vendor": "npm",
                "name": "cli",
                "version": npm.version(),
            }],
            "component": component,
        },
        "components": children
            .iter()
            .map(|node| cyclonedx_item(node, package_type))
            .collect::<Vec<_>>(),
        "dependencies": dependencies,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_sri_digests() {
        // sha512 of the empty string
        let sri = "sha512-z4PhNX7vuL3xVChQ1m2AB9Yg5AULVxXcg/SpIdNs6c5H0NE8XYXysP+DGNKHfuwvY7kxvUdBeoGlODJ6+SfaPg==";
        let (algorithm, digest) = integrity_digest(sri).unwrap();
        assert_eq!(algorithm, "sha512");
        assert_eq!(
            digest,
            "cf83e1357eefb8bdf1542850d66d8007d620e4050b5715dc83f4a921d36ce9ce47d0d13c5d85f2b0ff8318d2877eec2f63b931bd47417a81a538327af927da3e"
        );
    }

    #[test]
    fn generates_uuid_v4() {
        let uuid = random_uuid();
        assert!(crate::utils::validate_uuid::uuid_regex().is_match(&uuid));
    }

    #[test]
    fn builds_purls() {
        let scoped = SbomNode {
            name: "@scope/pkg".into(),
            version: "1.0.0".into(),
            ..Default::default()
        };
        assert_eq!(purl(&scoped), "pkg:npm/%40scope%2Fpkg@1.0.0");
        let plain = SbomNode {
            name: "pkg".into(),
            version: "1.0.0".into(),
            resolved: Some("git+https://github.com/a/b.git".into()),
            ..Default::default()
        };
        assert!(purl(&plain).contains("vcs_url=git+https:%2F%2Fgithub.com%2Fa%2Fb.git"), "{}", purl(&plain));
    }
}
