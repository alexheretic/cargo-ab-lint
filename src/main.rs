use anyhow::Context;
use cargo_metadata::{camino::Utf8PathBuf, PackageId};
use cargo_toml::Manifest;
use colored::Colorize;
use fs_err as fs;
use std::{env, str::FromStr};

fn main() -> anyhow::Result<()> {
    let fix = env::args().any(|a| a == "--fix");
    let dry_run = env::args().any(|a| a == "--dry-run");

    let meta = cargo_metadata::MetadataCommand::new().exec()?;

    let root_manifest = {
        let manifest = fs::read(&meta.workspace_root.join("Cargo.toml"))?;
        Manifest::from_slice(&manifest).with_context(|| format!("{}", meta.workspace_root))?
    };

    let mut something_to_fix = false;

    for member in meta.workspace_members {
        let Some(member_path) = member.manifest_path() else {
            continue;
        };
        let relative_member_path = env::current_dir()
            .ok()
            .and_then(|cwd| member_path.strip_prefix(&cwd).ok());
        eprintln!(
            "==> Checking {}",
            relative_member_path.unwrap_or(&member_path)
        );

        let (member_manifest, mut member_doc, toml_str) = {
            let toml = fs::read_to_string(&member_path)?;
            let manifest = Manifest::from_str(&toml).with_context(|| format!("{member_path}"))?;
            (manifest, toml.parse::<toml_edit::Document>()?, toml)
        };
        let has_fixes = lint_manifest(&root_manifest, &member_manifest, &mut member_doc);

        if fix && has_fixes {
            let fixed_toml = member_doc
                .to_string()
                .replace("workspace = true}", "workspace = true }");
            for diff in diff::lines(&toml_str, &fixed_toml) {
                match diff {
                    diff::Result::Left(old) => eprintln!("{}{}", "-".red(), old.red()),
                    diff::Result::Right(new) => eprintln!("{}{}", "+".green(), new.green()),
                    _ => {}
                }
            }
            if !dry_run {
                fs::write(&member_path, fixed_toml)?;
            }
        }
        something_to_fix |= has_fixes;
    }

    if !fix && something_to_fix {
        eprintln!(
            "{}{}",
            "Hint: To fix run with ".dimmed(),
            "--fix".dimmed().bold()
        );
        std::process::exit(1);
    }

    eprintln!("{}", "All good ✔".green());

    Ok(())
}

fn lint_manifest(root: &Manifest, member: &Manifest, doc: &mut toml_edit::Document) -> bool {
    let mut has_fixes = false;

    for (name, ws_dep) in root.workspace.iter().flat_map(|ws| &ws.dependencies) {
        if let Some(cargo_toml::Dependency::Inherited(dep)) = member.dependencies.get(name) {
            if dep.workspace {
                let doc_deps = &mut doc["dependencies"];

                has_fixes |= dependency_with_redundant_workspace_features(
                    name,
                    ws_dep,
                    dep,
                    doc_deps,
                    "dependency",
                );
                has_fixes |=
                    workspace_dependency_with_default_features_set(name, doc_deps, "dependency");
            }
        }
        if let Some(cargo_toml::Dependency::Inherited(dep)) = member.dev_dependencies.get(name) {
            if dep.workspace {
                let doc_devdeps = &mut doc["dev-dependencies"];

                has_fixes |= dependency_with_redundant_workspace_features(
                    name,
                    ws_dep,
                    dep,
                    doc_devdeps,
                    "dev-dependency",
                );
                has_fixes |= workspace_dependency_with_default_features_set(
                    name,
                    doc_devdeps,
                    "dev-dependency",
                );
            }
        }
    }

    has_fixes
}

/// workspace=true dependencies setting default-features has no effect.
fn workspace_dependency_with_default_features_set(
    dep_name: &str,
    doc_deps: &mut toml_edit::Item,
    item_name: &str,
) -> bool {
    if let Some(table) = doc_deps[dep_name].as_inline_table_mut() {
        let fixes = table.remove("default-features").is_some()
            || table.remove("default_features").is_some();

        if fixes {
            eprintln!(
                "{}",
                format!(
                    "Redundant default-features set in workspace {item_name} {}",
                    dep_name.bold()
                )
                .yellow()
            );
        }
        return fixes;
    }
    false
}

/// workspace=true dependencies do not need to restate the workspace features.
fn dependency_with_redundant_workspace_features(
    dep_name: &str,
    ws_dep: &cargo_toml::Dependency,
    dep: &cargo_toml::InheritedDependencyDetail,
    doc_deps: &mut toml_edit::Item,
    item_name: &str,
) -> bool {
    let mut has_fixes = false;

    let redundant_features: Vec<_> = dep
        .features
        .iter()
        .filter(|f| ws_dep.req_features().contains(f))
        .map(|s| s.as_str())
        .collect();

    if !redundant_features.is_empty() {
        eprintln!(
            "{}",
            format!(
                "Redundant feature(s) {} for workspace {item_name} {}",
                format!("{redundant_features:?}").bold(),
                dep_name.bold(),
            )
            .yellow()
        );
        has_fixes = true;

        let feats = doc_deps[dep_name]["features"].as_array_mut().unwrap();
        let rm_idx: Vec<_> = feats
            .iter()
            .enumerate()
            .filter(|(_, v)| {
                v.as_str()
                    .map_or(false, |s| redundant_features.contains(&s))
            })
            .map(|(idx, _)| idx)
            .collect();
        for idx in rm_idx.into_iter().rev() {
            feats.remove(idx);
        }

        if feats.is_empty() {
            doc_deps[dep_name]
                .as_inline_table_mut()
                .unwrap()
                .remove("features");
        }
    }

    has_fixes
}

trait PackageIdExt {
    fn manifest_path(&self) -> Option<Utf8PathBuf>;
}
impl PackageIdExt for PackageId {
    fn manifest_path(&self) -> Option<Utf8PathBuf> {
        let fidx = self.repr.find("(path+file://")?;
        let path = self.repr[fidx + "(path+file://".len()..].trim_end_matches(')');
        let path = Utf8PathBuf::from_str(path).ok()?;
        Some(path.join("Cargo.toml"))
    }
}
