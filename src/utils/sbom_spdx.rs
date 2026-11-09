//! Port of `lib/utils/sbom-spdx.js`: renders the install tree as an
//! SPDX 2.3 document.

use serde_json::{json, Map, Value};

use crate::npm::Npm;
use crate::utils::sbom_cyclonedx::{random_uuid, SbomEdge, SbomNode};

const SPDX_SCHEMA_VERSION: &str = "SPDX-2.3";
const SPDX_DATA_LICENSE: &str = "CC0-1.0";
const SPDX_IDENTIFIER: &str = "SPDXRef-DOCUMENT";
const NO_ASSERTION: &str = "NOASSERTION";

fn to_spdx_id(node: &SbomNode) -> String {
    let name = node.name.trim_start_matches('@').replace('/', ".");
    format!("SPDXRef-Package-{name}-{}", node.version)
}

fn package_str<'a>(node: &'a SbomNode, key: &str) -> Option<&'a str> {
    node.package.as_ref()?.get(key)?.as_str()
}

fn license(node: &SbomNode) -> Option<String> {
    let package = node.package.as_ref()?;
    if let Some(license) = package.get("license") {
        if let Some(text) = license.as_str() {
            return Some(text.to_string());
        }
        if let Some(kind) = license.get("type").and_then(Value::as_str) {
            return Some(kind.to_string());
        }
    }
    let licenses = package.get("licenses")?.as_array()?;
    let joined: Vec<&str> = licenses
        .iter()
        .filter_map(|l| l.as_str().or_else(|| l.get("type").and_then(Value::as_str)))
        .filter(|l| !l.is_empty())
        .collect();
    (!joined.is_empty()).then(|| joined.join(" OR "))
}

fn escaped_name(name: &str) -> String {
    match name.strip_prefix('@') {
        Some(rest) => match rest.split_once('/') {
            Some((scope, package)) => format!("%40{scope}%2f{package}"),
            None => format!("%40{rest}"),
        },
        None => name.to_string(),
    }
}

fn encoded_name(name: &str) -> String {
    let name = name.trim_start_matches('@');
    name.replace('/', "%2f")
}

fn checksum(node: &SbomNode) -> Option<(&str, String)> {
    let integrity = node.integrity.as_deref()?;
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

fn spdx_item(node: &SbomNode, package_type: Option<&str>) -> Value {
    let mut pkg = Map::new();
    pkg.insert("name".into(), json!(node.name));
    pkg.insert("SPDXID".into(), json!(to_spdx_id(node)));
    pkg.insert("versionInfo".into(), json!(node.version));
    if let Some(location) = &node.location {
        pkg.insert("packageFileName".into(), json!(location));
    }
    if let Some(description) = package_str(node, "description") {
        pkg.insert("description".into(), json!(description));
    }
    if let Some(package_type) = package_type {
        pkg.insert(
            "primaryPackagePurpose".into(),
            json!(package_type.to_uppercase()),
        );
    }
    let download = if node.is_link {
        NO_ASSERTION
    } else {
        node.resolved.as_deref().unwrap_or(NO_ASSERTION)
    };
    pkg.insert("downloadLocation".into(), json!(download));
    pkg.insert("filesAnalyzed".into(), json!(false));
    pkg.insert(
        "homepage".into(),
        json!(package_str(node, "homepage").unwrap_or(NO_ASSERTION)),
    );
    pkg.insert(
        "licenseDeclared".into(),
        json!(license(node).unwrap_or_else(|| NO_ASSERTION.to_string())),
    );
    let purl = format!(
        "pkg:npm/{}@{}",
        encoded_name(&node.name),
        node.version
    );
    pkg.insert(
        "externalRefs".into(),
        json!([{
            "referenceCategory": "PACKAGE-MANAGER",
            "referenceType": "purl",
            "referenceLocator": purl,
        }]),
    );
    if let Some((algorithm, digest)) = checksum(node) {
        pkg.insert(
            "checksums".into(),
            json!([{ "algorithm": algorithm.to_uppercase(), "checksumValue": digest }]),
        );
    }
    Value::Object(pkg)
}

fn relationship_type(edge: &SbomEdge) -> &'static str {
    match edge.edge_type.as_str() {
        "peer" => "PREREQUISITE_FOR",
        "optional" => "OPTIONAL_DEPENDENCY_OF",
        "dev" => "DEV_DEPENDENCY_OF",
        _ => "DEPENDENCY_OF",
    }
}

fn spdx_relationship(node: &SbomNode, edge: &SbomEdge, nodes: &[SbomNode]) -> Option<Value> {
    let target = nodes.get(edge.to)?;
    Some(json!({
        "spdxElementId": to_spdx_id(target),
        "relatedSpdxElement": to_spdx_id(node),
        "relationshipType": relationship_type(edge),
    }))
}

/// Builds the document. Links are resolved to their targets first.
pub fn spdx_output(npm: &dyn Npm, nodes: &[SbomNode], package_type: Option<&str>) -> Value {
    let Some(root) = nodes.iter().find(|node| node.is_root) else {
        return Value::Null;
    };
    let root_id = root.pkgid();
    let namespace = format!(
        "http://spdx.org/spdxdocs/{}-{}-{}",
        escaped_name(&root.name),
        root.version,
        random_uuid()
    );

    let mut children: Vec<&SbomNode> = Vec::new();
    for node in nodes.iter().filter(|node| !node.is_root && !node.is_link) {
        let id = to_spdx_id(node);
        if !children.iter().any(|seen| to_spdx_id(seen) == id) {
            children.push(node);
        }
    }

    let mut relationships = Vec::new();
    let mut seen: Vec<String> = Vec::new();
    for node in nodes {
        let key = to_spdx_id(node);
        if seen.contains(&key) {
            continue;
        }
        seen.push(key);
        let mut seen_rels: Vec<String> = Vec::new();
        for edge in &node.edges_out {
            let Some(rel) = spdx_relationship(node, edge, nodes) else {
                continue;
            };
            let rel_key = format!(
                "{}|{}|{}",
                rel["spdxElementId"], rel["relatedSpdxElement"], rel["relationshipType"]
            );
            if seen_rels.contains(&rel_key) {
                continue;
            }
            seen_rels.push(rel_key);
            relationships.push(rel);
        }
    }
    for (index, node) in nodes.iter().enumerate() {
        if !node.extraneous {
            continue;
        }
        let edge = SbomEdge {
            edge_type: "optional".into(),
            to: index,
        };
        if let Some(rel) = spdx_relationship(root, &edge, nodes) {
            relationships.push(rel);
        }
    }

    let mut all_relationships = vec![json!({
        "spdxElementId": SPDX_IDENTIFIER,
        "relatedSpdxElement": to_spdx_id(root),
        "relationshipType": "DESCRIBES",
    })];
    all_relationships.extend(relationships);

    let mut packages = vec![spdx_item(root, package_type)];
    packages.extend(
        children
            .iter()
            .map(|node| spdx_item(node, package_type)),
    );

    json!({
        "spdxVersion": SPDX_SCHEMA_VERSION,
        "dataLicense": SPDX_DATA_LICENSE,
        "SPDXID": SPDX_IDENTIFIER,
        "name": root_id,
        "documentNamespace": namespace,
        "creationInfo": {
            "created": chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            "creators": [format!("Tool: npm/cli-{}", npm.version())],
        },
        "documentDescribes": [to_spdx_id(root)],
        "packages": packages,
        "relationships": all_relationships,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitizes_scoped_names() {
        let node = SbomNode {
            name: "@scope/pkg".into(),
            version: "1.2.3".into(),
            ..Default::default()
        };
        assert_eq!(to_spdx_id(&node), "SPDXRef-Package-scope.pkg-1.2.3");
        assert_eq!(escaped_name("@scope/pkg"), "%40scope%2fpkg");
    }
}
