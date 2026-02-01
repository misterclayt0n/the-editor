//! Config crate install/build helpers.

use std::path::Path;

use eyre::{
  Result,
  WrapErr,
};

pub fn install_config_template() -> Result<()> {
  let config_dir = the_loader::config_dir();
  let template_dir = the_loader::config_template_dir();

  if config_dir.join("Cargo.toml").exists() {
    return Err(eyre::eyre!(
      "config crate already exists: {}",
      config_dir.display()
    ));
  }

  std::fs::create_dir_all(&config_dir)?;
  copy_dir_all(&template_dir, &config_dir)?;
  patch_template_manifest(&config_dir)?;

  Ok(())
}

pub fn build_config_binary() -> Result<()> {
  let config_dir = the_loader::config_dir();
  if !config_dir.join("Cargo.toml").exists() {
    return Err(eyre::eyre!(
      "config crate is missing Cargo.toml: {}",
      config_dir.display()
    ));
  }

  let package_name = read_package_name(&config_dir)?;

  let status = with_patched_repo_config_manifest(&config_dir, &package_name, || {
    std::process::Command::new("cargo")
      .arg("build")
      .arg("-p")
      .arg("the-term")
      .current_dir(the_loader::repo_root_dir())
      .status()
      .wrap_err("failed to build the-term")
  })?;

  if !status.success() {
    return Err(eyre::eyre!("build failed"));
  }

  Ok(())
}

fn copy_dir_all(src: &Path, dst: &Path) -> Result<()> {
  std::fs::create_dir_all(dst)?;
  for entry in std::fs::read_dir(src)? {
    let entry = entry?;
    let ty = entry.file_type()?;
    let src_path = entry.path();
    let dst_path = dst.join(entry.file_name());
    if ty.is_dir() {
      copy_dir_all(&src_path, &dst_path)?;
    } else {
      std::fs::copy(&src_path, &dst_path)?;
    }
  }
  Ok(())
}

fn patch_template_manifest(dir: &Path) -> Result<()> {
  let manifest = dir.join("Cargo.toml");
  let content = std::fs::read_to_string(&manifest)?;
  let repo_root = the_loader::repo_root_dir();
  let patched = content.replace("{{THE_EDITOR_REPO}}", repo_root.to_string_lossy().as_ref());
  std::fs::write(&manifest, patched)?;
  Ok(())
}

fn patch_repo_config_manifest(config_dir: &Path, package_name: &str) -> Result<String> {
  let manifest = the_loader::repo_config_dir().join("Cargo.toml");
  let content = std::fs::read_to_string(&manifest)?;
  let mut out = String::with_capacity(content.len() + 64);
  let mut in_deps = false;
  let mut replaced = false;

  for line in content.lines() {
    let trimmed = line.trim_start();
    if trimmed.starts_with('[') {
      if in_deps && !replaced {
        out.push_str("the-config-user = { path = \"");
        out.push_str(config_dir.to_string_lossy().as_ref());
        out.push_str("\", package = \"");
        out.push_str(package_name);
        out.push_str("\" }\n");
        replaced = true;
      }
      in_deps = trimmed == "[dependencies]";
      out.push_str(line);
      out.push('\n');
      continue;
    }

    if in_deps && trimmed.starts_with("the-config-user") {
      out.push_str("the-config-user = { path = \"");
      out.push_str(config_dir.to_string_lossy().as_ref());
      out.push_str("\", package = \"");
      out.push_str(package_name);
      out.push_str("\" }\n");
      replaced = true;
      continue;
    }

    out.push_str(line);
    out.push('\n');
  }

  if in_deps && !replaced {
    out.push_str("the-config-user = { path = \"");
    out.push_str(config_dir.to_string_lossy().as_ref());
    out.push_str("\", package = \"");
    out.push_str(package_name);
    out.push_str("\" }\n");
  }

  std::fs::write(&manifest, &out)?;
  Ok(content)
}

fn read_package_name(config_dir: &Path) -> Result<String> {
  let manifest = config_dir.join("Cargo.toml");
  let content = std::fs::read_to_string(&manifest)?;
  let mut in_package = false;

  for line in content.lines() {
    let trimmed = line.trim_start();
    if trimmed.starts_with('[') {
      in_package = trimmed == "[package]";
      continue;
    }

    if in_package && trimmed.starts_with("name") {
      let parts: Vec<&str> = trimmed.splitn(2, '=').collect();
      if parts.len() != 2 {
        break;
      }
      let value = parts[1].trim().trim_matches('"').to_string();
      if !value.is_empty() {
        if value == "the-config" {
          return Err(eyre::eyre!(
            "config crate package name must not be \"the-config\"; please rename it (e.g. to \"the-config-user\") in {}",
            manifest.display()
          ));
        }
        return Ok(value);
      }
    }
  }

  Err(eyre::eyre!(
    "failed to read package name from {}",
    manifest.display()
  ))
}

fn with_patched_repo_config_manifest<F>(
  config_dir: &Path,
  package_name: &str,
  action: F,
) -> Result<std::process::ExitStatus>
where
  F: FnOnce() -> Result<std::process::ExitStatus>,
{
  let manifest = the_loader::repo_config_dir().join("Cargo.toml");
  let original = patch_repo_config_manifest(config_dir, package_name)?;
  let result = action();
  let _ = std::fs::write(&manifest, original);
  result
}
