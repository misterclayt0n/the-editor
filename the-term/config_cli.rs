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
  let repo_config_dir = the_loader::repo_config_dir();

  if config_dir.join("Cargo.toml").exists() {
    sync_config_crate(&config_dir, &repo_config_dir)?;
  }

  let status = std::process::Command::new("cargo")
    .arg("build")
    .arg("-p")
    .arg("the-term")
    .current_dir(the_loader::repo_root_dir())
    .status()
    .wrap_err("failed to build the-term")?;

  if !status.success() {
    return Err(eyre::eyre!("build failed"));
  }

  Ok(())
}

fn sync_config_crate(src: &Path, dst: &Path) -> Result<()> {
  let src_manifest = src.join("Cargo.toml");
  let src_src = src.join("src");

  if !src_manifest.exists() || !src_src.exists() {
    return Err(eyre::eyre!(
      "config crate is missing Cargo.toml or src/: {}",
      src.display()
    ));
  }

  std::fs::create_dir_all(dst)?;
  let manifest_content = std::fs::read_to_string(&src_manifest)?;
  let manifest_content = patch_package_name(&manifest_content, "the-config");
  std::fs::write(dst.join("Cargo.toml"), manifest_content)?;

  let dst_src = dst.join("src");
  if dst_src.exists() {
    std::fs::remove_dir_all(&dst_src)?;
  }
  copy_dir_all(&src_src, &dst_src)?;

  let src_build = src.join("build.rs");
  if src_build.exists() {
    std::fs::copy(&src_build, dst.join("build.rs"))?;
  } else {
    let dst_build = dst.join("build.rs");
    if dst_build.exists() {
      std::fs::remove_file(dst_build)?;
    }
  }

  sync_optional_dir(src, dst, "assets")?;
  sync_optional_dir(src, dst, "resources")?;
  sync_optional_dir(src, dst, "data")?;
  sync_optional_dir(src, dst, "templates")?;
  sync_optional_dir(src, dst, "queries")?;

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

fn sync_optional_dir(src_root: &Path, dst_root: &Path, name: &str) -> Result<()> {
  let src_dir = src_root.join(name);
  let dst_dir = dst_root.join(name);
  if !src_dir.exists() {
    if dst_dir.exists() {
      std::fs::remove_dir_all(dst_dir)?;
    }
    return Ok(());
  }

  if dst_dir.exists() {
    std::fs::remove_dir_all(&dst_dir)?;
  }
  copy_dir_all(&src_dir, &dst_dir)?;
  Ok(())
}

fn patch_package_name(content: &str, name: &str) -> String {
  let mut out = String::with_capacity(content.len() + 16);
  let mut in_package = false;
  let mut replaced = false;

  for line in content.lines() {
    let trimmed = line.trim_start();
    if trimmed.starts_with('[') {
      if in_package && !replaced {
        out.push_str("name = \"");
        out.push_str(name);
        out.push_str("\"\n");
        replaced = true;
      }
      in_package = trimmed == "[package]";
      out.push_str(line);
      out.push('\n');
      continue;
    }

    if in_package && trimmed.starts_with("name") {
      let prefix_len = line.len().saturating_sub(trimmed.len());
      let prefix = &line[..prefix_len];
      out.push_str(prefix);
      out.push_str("name = \"");
      out.push_str(name);
      out.push_str("\"\n");
      replaced = true;
      continue;
    }

    out.push_str(line);
    out.push('\n');
  }

  if in_package && !replaced {
    out.push_str("name = \"");
    out.push_str(name);
    out.push_str("\"\n");
  }

  out
}
