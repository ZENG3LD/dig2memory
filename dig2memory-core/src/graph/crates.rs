use crate::error::CoreError;
use crate::types::{CrateNode, DepEdge};
use std::path::Path;
use toml::Value;

/// Parse the cargo workspace rooted at `root`, returning crate nodes and dependency edges.
pub fn parse_cargo_workspace(
    root: &Path,
    workspace_id: &str,
) -> Result<(Vec<CrateNode>, Vec<DepEdge>), CoreError> {
    let manifest_path = root.join("Cargo.toml");
    let manifest_str = std::fs::read_to_string(&manifest_path).map_err(|e| CoreError::Io {
        path: manifest_path.to_string_lossy().into_owned(),
        source: e,
    })?;

    let root_value: Value =
        toml::from_str(&manifest_str).map_err(|e| CoreError::Manifest {
            path: manifest_path.to_string_lossy().into_owned(),
            source: e,
        })?;

    let mut members: Vec<String> = Vec::new();

    if let Some(ws) = root_value.get("workspace") {
        if let Some(mem_arr) = ws.get("members").and_then(|v| v.as_array()) {
            for item in mem_arr {
                if let Some(pattern) = item.as_str() {
                    // Simple glob expansion: patterns like "crates/*"
                    let expanded = expand_member_glob(root, pattern);
                    members.extend(expanded);
                }
            }
        }
    } else {
        // Single-crate workspace — treat root itself as the member.
        members.push(".".to_owned());
    }

    let mut nodes: Vec<CrateNode> = Vec::new();
    let mut edges: Vec<DepEdge> = Vec::new();

    for member_rel in &members {
        let member_path = root.join(member_rel);
        let member_manifest = member_path.join("Cargo.toml");
        let member_str =
            match std::fs::read_to_string(&member_manifest) {
                Ok(s) => s,
                Err(_) => continue,
            };

        let member_value: Value = match toml::from_str(&member_str) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let pkg = match member_value.get("package") {
            Some(p) => p,
            None => continue,
        };

        let name = pkg
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_owned();
        if name.is_empty() {
            continue;
        }

        let version = pkg
            .get("version")
            .and_then(|v| v.as_str())
            .map(|s| s.to_owned());

        let manifest_path_str = member_manifest.to_string_lossy().replace('\\', "/");

        nodes.push(CrateNode {
            workspace_id: workspace_id.to_owned(),
            name: name.clone(),
            version,
            manifest_path: manifest_path_str,
        });

        // Extract path dependencies.
        if let Some(deps) = member_value.get("dependencies") {
            collect_path_deps(deps, workspace_id, &name, &mut edges);
        }
        if let Some(deps) = member_value.get("dev-dependencies") {
            collect_path_deps(deps, workspace_id, &name, &mut edges);
        }
        if let Some(deps) = member_value.get("build-dependencies") {
            collect_path_deps(deps, workspace_id, &name, &mut edges);
        }
    }

    Ok((nodes, edges))
}

fn collect_path_deps(
    deps: &Value,
    workspace_id: &str,
    from_crate: &str,
    edges: &mut Vec<DepEdge>,
) {
    if let Some(table) = deps.as_table() {
        for (dep_name, dep_val) in table {
            let is_path = dep_val
                .as_table()
                .and_then(|t| t.get("path"))
                .is_some();
            edges.push(DepEdge {
                workspace_id: workspace_id.to_owned(),
                from_crate: from_crate.to_owned(),
                to_crate: dep_name.clone(),
                is_path_dep: is_path,
            });
        }
    }
}

/// Expand a member glob pattern to actual directory paths relative to root.
fn expand_member_glob(root: &Path, pattern: &str) -> Vec<String> {
    // Handle simple "prefix/*" patterns.
    if let Some(prefix) = pattern.strip_suffix("/*") {
        let dir = root.join(prefix);
        let mut result = Vec::new();
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() && path.join("Cargo.toml").exists() {
                    let rel = format!("{}/{}", prefix, entry.file_name().to_string_lossy());
                    result.push(rel);
                }
            }
        }
        result
    } else if pattern.contains('*') {
        // Unsupported glob pattern — skip.
        Vec::new()
    } else {
        vec![pattern.to_owned()]
    }
}
