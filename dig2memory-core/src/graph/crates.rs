use cargo_metadata::MetadataCommand;
use crate::error::CoreError;
use crate::types::{CrateNode, DepEdge};
use std::path::Path;

/// Parse the cargo workspace rooted at `root` using `cargo metadata`, returning
/// crate nodes and dependency edges for workspace members only.
pub fn parse_cargo_workspace(
    root: &Path,
    workspace_id: &str,
) -> Result<(Vec<CrateNode>, Vec<DepEdge>), CoreError> {
    let manifest_path = root.join("Cargo.toml");
    if !manifest_path.exists() {
        return Ok((vec![], vec![]));
    }

    let metadata = MetadataCommand::new()
        .manifest_path(&manifest_path)
        .no_deps()
        .exec()
        .map_err(|e| CoreError::Manifest {
            path: manifest_path.to_string_lossy().replace('\\', "/"),
            reason: e.to_string(),
        })?;

    let root_str = root.to_string_lossy().replace('\\', "/");
    // Ensure root_str has no trailing slash for consistent prefix stripping.
    let root_str = root_str.trim_end_matches('/');

    let mut nodes = Vec::new();
    let mut edges = Vec::new();

    for pkg in &metadata.packages {
        let pkg_manifest = pkg.manifest_path.to_string().replace('\\', "/");
        // source_dir = directory of the manifest, relative to workspace root.
        let pkg_dir = pkg_manifest
            .strip_suffix("/Cargo.toml")
            .unwrap_or(&pkg_manifest);
        let source_dir = if let Some(stripped) = pkg_dir.strip_prefix(root_str) {
            stripped.trim_start_matches('/').to_string()
        } else {
            pkg_dir.to_string()
        };

        // manifest_path stored relative, e.g. "zengeld-terminal/crates/app/Cargo.toml"
        let manifest_rel = if let Some(stripped) = pkg_manifest.strip_prefix(root_str) {
            stripped.trim_start_matches('/').to_string()
        } else {
            pkg_manifest.clone()
        };

        let is_workspace_member = metadata.workspace_members.contains(&pkg.id);

        nodes.push(CrateNode {
            workspace_id: workspace_id.to_string(),
            name: pkg.name.clone(),
            version: pkg.version.to_string(),
            manifest_path: manifest_rel,
            source_dir,
            is_workspace_member,
        });

        // Emit dependency edges for all declared deps.
        for dep in &pkg.dependencies {
            let dep_path = dep
                .path
                .as_ref()
                .map(|p| p.to_string().replace('\\', "/"));

            // Make dep_path relative to workspace root when possible.
            let dep_path_rel = dep_path.map(|p| {
                if let Some(stripped) = p.strip_prefix(root_str) {
                    stripped.trim_start_matches('/').to_string()
                } else {
                    p
                }
            });

            edges.push(DepEdge {
                workspace_id: workspace_id.to_string(),
                from_crate: pkg.name.clone(),
                to_crate: dep.name.clone(),
                is_path_dep: dep_path_rel.is_some(),
                dep_path: dep_path_rel,
            });
        }
    }

    Ok((nodes, edges))
}
