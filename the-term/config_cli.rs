//! Config crate install/build helpers.

use std::{
  env,
  ffi::OsString,
  fmt,
  fs,
  path::{
    Path,
    PathBuf,
  },
  process::{
    Command,
    ExitStatus,
  },
};

use clap::ValueEnum;
use eyre::{
  Result,
  WrapErr,
  bail,
  eyre,
};
use toml::{
  Value,
  map::Map,
};

const BUILD_LAYOUT_VERSION: &str = "1";
const GENERATED_CONFIG_PACKAGE: &str = "the-config";
const GENERATED_CONFIG_BRIDGE_CRATE: &str = "the-config";
const GENERATED_TARGET_CRATE: &str = "the-term";
const RESERVED_EXTERNAL_CONFIG_PACKAGE: &str = "the-config";
const SIGNATURE_FILE: &str = ".config-build-signature";

#[derive(Clone, Debug)]
pub struct ConfigInitOptions {
  pub config_dir:   Option<PathBuf>,
  pub package_name: Option<String>,
}

#[derive(Clone, Debug)]
pub struct ConfigPathOptions {
  pub config_dir: Option<PathBuf>,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, ValueEnum)]
pub enum ConfigTarget {
  Term,
}

impl Default for ConfigTarget {
  fn default() -> Self {
    Self::Term
  }
}

impl fmt::Display for ConfigTarget {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str(self.as_str())
  }
}

impl ConfigTarget {
  fn as_str(self) -> &'static str {
    match self {
      Self::Term => "term",
    }
  }

  fn crate_dir_name(self) -> &'static str {
    match self {
      Self::Term => GENERATED_TARGET_CRATE,
    }
  }

  fn package_name(self) -> &'static str {
    match self {
      Self::Term => GENERATED_TARGET_CRATE,
    }
  }

  fn binary_name(self) -> &'static str {
    match self {
      Self::Term => "the-editor",
    }
  }

  fn binary_file_name(self) -> String {
    format!("{}{}", self.binary_name(), env::consts::EXE_SUFFIX)
  }

  fn crate_dir(self, repo_root: &Path) -> PathBuf {
    repo_root.join(self.crate_dir_name())
  }

  fn manifest_path(self, repo_root: &Path) -> PathBuf {
    self.crate_dir(repo_root).join("Cargo.toml")
  }

  fn install_path(self, config_dir: &Path) -> PathBuf {
    config_dir.join("bin").join(self.binary_file_name())
  }
}

#[derive(Clone, Debug)]
pub struct ConfigBuildOptions {
  pub config_dir: Option<PathBuf>,
  pub target:     ConfigTarget,
  pub release:    bool,
  pub out_path:   Option<PathBuf>,
  pub install:    bool,
}

#[derive(Clone, Debug)]
pub struct ConfigRunOptions {
  pub config_dir: Option<PathBuf>,
  pub target:     ConfigTarget,
  pub release:    bool,
  pub args:       Vec<OsString>,
}

#[derive(Copy, Clone, Debug)]
enum CargoBuildMode {
  Check,
  Build,
}

impl CargoBuildMode {
  fn cargo_subcommand(self) -> &'static str {
    match self {
      Self::Check => "check",
      Self::Build => "build",
    }
  }

  fn action_label(self) -> &'static str {
    match self {
      Self::Check => "check",
      Self::Build => "build",
    }
  }
}

#[derive(Clone, Debug)]
struct ConfigCliContext {
  repo_root:          PathBuf,
  cache_dir:          PathBuf,
  default_config_dir: PathBuf,
  template_dir:       PathBuf,
}

impl ConfigCliContext {
  fn discover() -> Self {
    let repo_root = normalize_existing_path(&the_loader::repo_root_dir());
    Self {
      template_dir: repo_root.join("the-config").join("template"),
      repo_root,
      cache_dir: the_loader::cache_dir(),
      default_config_dir: the_loader::config_dir(),
    }
  }

  fn resolve_config_dir(&self, override_dir: Option<&Path>) -> Result<PathBuf> {
    resolve_config_dir_from_sources(
      override_dir,
      Some(self.default_config_dir.as_path()),
      env::current_dir()
        .wrap_err("failed to read current directory")?
        .as_path(),
    )
  }

  fn build_root(&self, config_dir: &Path, target: ConfigTarget) -> PathBuf {
    let input = format!(
      "{}\n{}\n{}\n",
      self.repo_root.display(),
      config_dir.display(),
      target.as_str()
    );
    let hash = stable_hash_hex(input.as_bytes());
    self.cache_dir.join("config-build").join(hash)
  }
}

#[derive(Clone, Debug)]
struct ExternalConfig {
  dir:           PathBuf,
  manifest_path: PathBuf,
  package_name:  String,
}

#[derive(Clone, Debug)]
struct BuildHarness {
  root_dir:          PathBuf,
  config_bridge_dir: PathBuf,
  target_dir:        PathBuf,
  signature_path:    PathBuf,
}

impl BuildHarness {
  fn new(ctx: &ConfigCliContext, config_dir: &Path, target: ConfigTarget) -> Self {
    let root_dir = ctx.build_root(config_dir, target);
    Self {
      config_bridge_dir: root_dir.join(GENERATED_CONFIG_BRIDGE_CRATE),
      target_dir: root_dir.join(target.crate_dir_name()),
      signature_path: root_dir.join(SIGNATURE_FILE),
      root_dir,
    }
  }

  fn manifest_path(&self) -> PathBuf {
    self.root_dir.join("Cargo.toml")
  }

  fn target_manifest_path(&self) -> PathBuf {
    self.target_dir.join("Cargo.toml")
  }

  fn config_manifest_path(&self) -> PathBuf {
    self.config_bridge_dir.join("Cargo.toml")
  }

  fn config_lib_path(&self) -> PathBuf {
    self.config_bridge_dir.join("src").join("lib.rs")
  }

  fn output_binary_path(&self, target: ConfigTarget, release: bool) -> PathBuf {
    let profile = if release { "release" } else { "debug" };
    self
      .root_dir
      .join("target")
      .join(profile)
      .join(target.binary_file_name())
  }

  fn lockfile_path(&self) -> PathBuf {
    self.root_dir.join("Cargo.lock")
  }

  fn required_paths(&self) -> [PathBuf; 4] {
    [
      self.manifest_path(),
      self.config_manifest_path(),
      self.config_lib_path(),
      self.target_manifest_path(),
    ]
  }
}

#[derive(Clone, Debug)]
struct HarnessInputs {
  repo_manifest:   String,
  repo_lock:       Option<String>,
  target_manifest: String,
  config_manifest: String,
}

pub fn init_config_template(options: ConfigInitOptions) -> Result<()> {
  let ctx = ConfigCliContext::discover();
  let config_dir = ctx.resolve_config_dir(options.config_dir.as_deref())?;

  if config_dir.join("Cargo.toml").exists() {
    return Err(eyre!(
      "config crate already exists: {}",
      config_dir.display()
    ));
  }

  fs::create_dir_all(&config_dir)
    .wrap_err_with(|| format!("failed to create config directory {}", config_dir.display()))?;
  copy_dir_all(&ctx.template_dir, &config_dir)?;
  if let Some(package_name) = options.package_name.as_deref() {
    write_package_name(&config_dir.join("Cargo.toml"), package_name)?;
  }

  println!("created config crate at {}", config_dir.display());
  Ok(())
}

pub fn print_config_path(options: ConfigPathOptions) -> Result<()> {
  let ctx = ConfigCliContext::discover();
  let config_dir = ctx.resolve_config_dir(options.config_dir.as_deref())?;
  println!("{}", config_dir.display());
  Ok(())
}

pub fn print_config_status(options: ConfigPathOptions) -> Result<()> {
  let ctx = ConfigCliContext::discover();
  let config_dir = ctx.resolve_config_dir(options.config_dir.as_deref())?;
  let manifest_path = config_dir.join("Cargo.toml");
  let install_path = ConfigTarget::Term.install_path(&config_dir);
  let build_root = ctx.build_root(&config_dir, ConfigTarget::Term);
  let harness = BuildHarness::new(&ctx, &config_dir, ConfigTarget::Term);
  let exists = config_dir.exists();
  let manifest_exists = manifest_path.exists();
  let package_name = read_package_name(&manifest_path).ok();
  let harness_state = if let Some(package_name) = package_name.as_deref() {
    let inputs = load_harness_inputs(&ctx, &config_dir, ConfigTarget::Term)?;
    let config = ExternalConfig {
      dir:           config_dir.clone(),
      manifest_path: manifest_path.clone(),
      package_name:  package_name.to_string(),
    };
    if is_harness_ready(&ctx, &harness, &config, ConfigTarget::Term, &inputs)? {
      "ready"
    } else {
      "will be regenerated"
    }
  } else {
    "will be regenerated"
  };

  println!("Config status");
  println!("config dir: {}", config_dir.display());
  println!("exists: {}", yes_no(exists));
  println!("manifest: {}", yes_no(manifest_exists));
  match read_package_name(&manifest_path) {
    Ok(package_name) => println!("package: {}", package_name),
    Err(error) if manifest_exists => println!("package: unavailable ({error})"),
    Err(_) => println!("package: unavailable"),
  }
  println!("default target: {}", ConfigTarget::Term);
  println!("repo root: {}", ctx.repo_root.display());
  println!("build root: {}", build_root.display());
  println!("install path: {}", install_path.display());
  println!("installed: {}", yes_no(install_path.exists()));
  println!("build harness: {harness_state}");
  println!(
    "next: {}",
    recommended_next_command(&config_dir, manifest_exists, package_name.is_some())
  );

  Ok(())
}

pub fn check_config_binary(options: ConfigBuildOptions) -> Result<()> {
  run_config_cargo(options, CargoBuildMode::Check).map(|_| ())
}

pub fn build_config_binary(options: ConfigBuildOptions) -> Result<()> {
  let built_binary = run_config_cargo(options.clone(), CargoBuildMode::Build)?;
  if let Some(out_path) = options.out_path.as_deref() {
    let out_path = resolve_output_destination(out_path, options.target)?;
    copy_built_binary(&built_binary, &out_path)?;
    println!("copied binary: {}", out_path.display());
  }
  if options.install {
    let ctx = ConfigCliContext::discover();
    let config_dir = ctx.resolve_config_dir(options.config_dir.as_deref())?;
    let install_path = options.target.install_path(&config_dir);
    copy_built_binary(&built_binary, &install_path)?;
    println!("installed binary: {}", install_path.display());
  }
  Ok(())
}

pub fn run_config_binary(options: ConfigRunOptions) -> Result<ExitStatus> {
  let built_binary = run_config_cargo(
    ConfigBuildOptions {
      config_dir: options.config_dir.clone(),
      target:     options.target,
      release:    options.release,
      out_path:   None,
      install:    false,
    },
    CargoBuildMode::Build,
  )?;

  eprintln!("running configured binary: {}", built_binary.display());
  Command::new(&built_binary)
    .args(&options.args)
    .status()
    .wrap_err_with(|| format!("failed to run configured binary {}", built_binary.display()))
}

fn run_config_cargo(options: ConfigBuildOptions, mode: CargoBuildMode) -> Result<PathBuf> {
  let ctx = ConfigCliContext::discover();
  let config_dir = ctx.resolve_config_dir(options.config_dir.as_deref())?;
  let config = load_external_config(&config_dir)?;
  let inputs = load_harness_inputs(&ctx, &config_dir, options.target)?;
  let harness = ensure_build_harness(&ctx, &config, options.target, &inputs)?;

  print_build_context(&ctx, &config, options.target, &harness.root_dir, mode);
  let status = run_cargo(&harness, &options, mode)?;
  if !status.success() {
    eprintln!(
      "{} failed for config {}",
      mode.action_label(),
      config.package_name
    );
    eprintln!("config dir: {}", config.dir.display());
    eprintln!("target: {}", options.target);
    eprintln!("repo root: {}", ctx.repo_root.display());
    eprintln!("build root: {}", harness.root_dir.display());
    return Err(eyre!("cargo {} failed", mode.action_label()));
  }

  let built_binary = harness.output_binary_path(options.target, options.release);
  if matches!(mode, CargoBuildMode::Build) {
    println!("built binary: {}", built_binary.display());
  }

  Ok(built_binary)
}

fn print_build_context(
  ctx: &ConfigCliContext,
  config: &ExternalConfig,
  target: ConfigTarget,
  build_root: &Path,
  mode: CargoBuildMode,
) {
  eprintln!(
    "running cargo {} for config {}",
    mode.action_label(),
    config.package_name
  );
  eprintln!("config dir: {}", config.dir.display());
  eprintln!("package: {}", config.package_name);
  eprintln!("target: {}", target);
  eprintln!("repo root: {}", ctx.repo_root.display());
  eprintln!("build root: {}", build_root.display());
}

fn run_cargo(
  harness: &BuildHarness,
  options: &ConfigBuildOptions,
  mode: CargoBuildMode,
) -> Result<ExitStatus> {
  let mut command = Command::new("cargo");
  command
    .arg(mode.cargo_subcommand())
    .arg("--manifest-path")
    .arg(harness.manifest_path())
    .arg("-p")
    .arg(options.target.package_name())
    .current_dir(&harness.root_dir);

  if options.release {
    command.arg("--release");
  }

  command.status().wrap_err_with(|| {
    format!(
      "failed to invoke cargo {} for {}",
      mode.action_label(),
      harness.root_dir.display()
    )
  })
}

fn load_external_config(config_dir: &Path) -> Result<ExternalConfig> {
  let manifest_path = config_dir.join("Cargo.toml");
  if !config_dir.exists() {
    bail!(
      "config crate is missing: {}\nrun `the-editor config init` to create it",
      config_dir.display()
    );
  }
  if !manifest_path.exists() {
    bail!(
      "config crate is missing Cargo.toml: {}\nrun `the-editor config init` to create it",
      config_dir.display()
    );
  }

  let package_name = read_package_name(&manifest_path)?;
  Ok(ExternalConfig {
    dir: config_dir.to_path_buf(),
    manifest_path,
    package_name,
  })
}

fn ensure_build_harness(
  ctx: &ConfigCliContext,
  config: &ExternalConfig,
  target: ConfigTarget,
  inputs: &HarnessInputs,
) -> Result<BuildHarness> {
  let harness = BuildHarness::new(ctx, &config.dir, target);
  if is_harness_ready(ctx, &harness, config, target, inputs)? {
    return Ok(harness);
  }

  generate_build_harness(ctx, &harness, config, target, inputs)?;
  Ok(harness)
}

fn is_harness_ready(
  ctx: &ConfigCliContext,
  harness: &BuildHarness,
  config: &ExternalConfig,
  target: ConfigTarget,
  inputs: &HarnessInputs,
) -> Result<bool> {
  for path in harness.required_paths() {
    if !path.exists() {
      return Ok(false);
    }
  }

  let existing_signature = match fs::read_to_string(&harness.signature_path) {
    Ok(signature) => signature,
    Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
    Err(error) => return Err(error).wrap_err("failed to read build harness signature"),
  };

  match inputs.repo_lock.as_ref() {
    Some(_) if !harness.lockfile_path().exists() => return Ok(false),
    Some(_) => {},
    None if harness.lockfile_path().exists() => return Ok(false),
    None => {},
  }

  Ok(existing_signature == harness_signature(ctx, config, target, inputs))
}

fn generate_build_harness(
  ctx: &ConfigCliContext,
  harness: &BuildHarness,
  config: &ExternalConfig,
  target: ConfigTarget,
  inputs: &HarnessInputs,
) -> Result<()> {
  fs::create_dir_all(&harness.config_bridge_dir)?;
  fs::create_dir_all(
    harness
      .config_lib_path()
      .parent()
      .unwrap_or(&harness.config_bridge_dir),
  )?;
  fs::create_dir_all(&harness.target_dir)?;

  fs::write(harness.manifest_path(), render_workspace_manifest(ctx)?)?;
  fs::write(
    harness.config_manifest_path(),
    render_config_bridge_manifest(ctx, config)?,
  )?;
  fs::write(harness.config_lib_path(), render_config_bridge_lib())?;
  fs::write(
    harness.target_manifest_path(),
    render_target_manifest(ctx, target, &harness.config_bridge_dir)?,
  )?;
  fs::write(
    &harness.signature_path,
    harness_signature(ctx, config, target, inputs),
  )?;
  sync_workspace_lockfile(ctx, harness)?;

  Ok(())
}

fn load_harness_inputs(
  ctx: &ConfigCliContext,
  config_dir: &Path,
  target: ConfigTarget,
) -> Result<HarnessInputs> {
  Ok(HarnessInputs {
    repo_manifest:   fs::read_to_string(ctx.repo_root.join("Cargo.toml"))
      .wrap_err("failed to read workspace Cargo.toml")?,
    repo_lock:       fs::read_to_string(ctx.repo_root.join("Cargo.lock")).ok(),
    target_manifest: fs::read_to_string(target.manifest_path(&ctx.repo_root))
      .wrap_err("failed to read target Cargo.toml")?,
    config_manifest: fs::read_to_string(config_dir.join("Cargo.toml"))
      .wrap_err("failed to read config Cargo.toml")?,
  })
}

fn render_workspace_manifest(ctx: &ConfigCliContext) -> Result<String> {
  let manifest_path = ctx.repo_root.join("Cargo.toml");
  let manifest = read_manifest_value(&manifest_path)?;
  let workspace = manifest
    .get("workspace")
    .and_then(Value::as_table)
    .cloned()
    .ok_or_else(|| eyre!("workspace manifest is missing [workspace]"))?;

  let mut generated = Map::new();
  let mut workspace = workspace;
  workspace.insert(
    "members".to_string(),
    Value::Array(vec![
      Value::String(GENERATED_CONFIG_BRIDGE_CRATE.to_string()),
      Value::String(GENERATED_TARGET_CRATE.to_string()),
    ]),
  );
  generated.insert("workspace".to_string(), Value::Table(workspace));

  if let Some(patch) = manifest.get("patch").cloned() {
    generated.insert("patch".to_string(), patch);
  }

  inject_workspace_member_patches(&mut generated, ctx, &[
    GENERATED_CONFIG_PACKAGE,
    GENERATED_TARGET_CRATE,
  ])?;

  toml::to_string_pretty(&Value::Table(generated))
    .wrap_err("failed to render generated workspace manifest")
}

fn inject_workspace_member_patches(
  manifest: &mut Map<String, Value>,
  ctx: &ConfigCliContext,
  excluded_packages: &[&str],
) -> Result<()> {
  let patches = manifest
    .entry("patch".to_string())
    .or_insert_with(|| Value::Table(Map::new()))
    .as_table_mut()
    .ok_or_else(|| eyre!("generated manifest [patch] is not a table"))?;
  let crates_io = patches
    .entry("crates-io".to_string())
    .or_insert_with(|| Value::Table(Map::new()))
    .as_table_mut()
    .ok_or_else(|| eyre!("generated manifest [patch.crates-io] is not a table"))?;

  for member in workspace_members(ctx)? {
    if excluded_packages
      .iter()
      .any(|package| *package == member.package_name)
    {
      continue;
    }
    crates_io.insert(
      member.package_name,
      Value::Table(Map::from_iter([(
        "path".to_string(),
        Value::String(member.manifest_dir.to_string_lossy().into_owned()),
      )])),
    );
  }

  Ok(())
}

#[derive(Clone, Debug)]
struct WorkspaceMember {
  package_name: String,
  manifest_dir: PathBuf,
}

fn workspace_members(ctx: &ConfigCliContext) -> Result<Vec<WorkspaceMember>> {
  let manifest_path = ctx.repo_root.join("Cargo.toml");
  let manifest = read_manifest_value(&manifest_path)?;
  let members = manifest
    .get("workspace")
    .and_then(Value::as_table)
    .and_then(|table| table.get("members"))
    .and_then(Value::as_array)
    .ok_or_else(|| eyre!("workspace manifest is missing workspace.members"))?;

  let mut out = Vec::with_capacity(members.len());
  for member in members {
    let Some(member_path) = member.as_str() else {
      bail!("workspace member entry must be a string");
    };
    let manifest_dir = ctx.repo_root.join(member_path);
    let manifest_path = manifest_dir.join("Cargo.toml");
    let package_name = read_manifest_package_name(&manifest_path)?;
    out.push(WorkspaceMember {
      package_name,
      manifest_dir,
    });
  }

  Ok(out)
}

fn render_config_bridge_manifest(
  ctx: &ConfigCliContext,
  config: &ExternalConfig,
) -> Result<String> {
  let mut manifest = Map::new();
  manifest.insert(
    "package".to_string(),
    Value::Table(Map::from_iter([
      (
        "name".to_string(),
        Value::String(GENERATED_CONFIG_PACKAGE.to_string()),
      ),
      ("version".to_string(), Value::String("0.1.0".to_string())),
      ("edition".to_string(), Value::String("2024".to_string())),
    ])),
  );
  manifest.insert(
    "lib".to_string(),
    Value::Table(Map::from_iter([(
      "path".to_string(),
      Value::String("src/lib.rs".to_string()),
    )])),
  );
  manifest.insert(
    "dependencies".to_string(),
    Value::Table(Map::from_iter([
      (
        "the-config-user".to_string(),
        Value::Table(Map::from_iter([
          (
            "path".to_string(),
            Value::String(config.dir.to_string_lossy().into_owned()),
          ),
          (
            "package".to_string(),
            Value::String(config.package_name.clone()),
          ),
        ])),
      ),
      (
        "the-default".to_string(),
        Value::Table(Map::from_iter([(
          "path".to_string(),
          Value::String(
            ctx
              .repo_root
              .join("the-default")
              .to_string_lossy()
              .into_owned(),
          ),
        )])),
      ),
    ])),
  );

  toml::to_string_pretty(&Value::Table(manifest))
    .wrap_err("failed to render generated config bridge manifest")
}

fn render_config_bridge_lib() -> &'static str {
  "//! Generated config bridge crate.\n\npub use the_config_user::*;\n"
}

fn render_target_manifest(
  ctx: &ConfigCliContext,
  target: ConfigTarget,
  config_bridge_dir: &Path,
) -> Result<String> {
  let manifest_path = target.manifest_path(&ctx.repo_root);
  let crate_dir = target.crate_dir(&ctx.repo_root);
  let mut manifest = read_manifest_value(&manifest_path)?;
  rewrite_target_manifest_paths(
    manifest
      .as_table_mut()
      .ok_or_else(|| eyre!("target manifest root is not a table"))?,
    &crate_dir,
    config_bridge_dir,
  )?;

  toml::to_string_pretty(&manifest).wrap_err("failed to render generated target manifest")
}

fn rewrite_target_manifest_paths(
  manifest: &mut Map<String, Value>,
  crate_dir: &Path,
  config_bridge_dir: &Path,
) -> Result<()> {
  rewrite_package_build_path(manifest, crate_dir);
  rewrite_target_source_path(manifest, "lib", crate_dir);
  rewrite_target_source_paths(manifest, "bin", crate_dir);
  rewrite_target_source_paths(manifest, "example", crate_dir);
  rewrite_target_source_paths(manifest, "test", crate_dir);
  rewrite_target_source_paths(manifest, "bench", crate_dir);
  rewrite_dependency_group(manifest, "dependencies", crate_dir, config_bridge_dir);
  rewrite_dependency_group(manifest, "build-dependencies", crate_dir, config_bridge_dir);
  rewrite_dependency_group(manifest, "dev-dependencies", crate_dir, config_bridge_dir);
  rewrite_target_specific_dependency_groups(manifest, crate_dir, config_bridge_dir)?;
  Ok(())
}

fn rewrite_package_build_path(manifest: &mut Map<String, Value>, crate_dir: &Path) {
  let Some(package) = manifest.get_mut("package").and_then(Value::as_table_mut) else {
    return;
  };
  if let Some(build) = package.get_mut("build") {
    absolutize_string_value(build, crate_dir);
  }
}

fn rewrite_target_source_path(manifest: &mut Map<String, Value>, key: &str, crate_dir: &Path) {
  let Some(table) = manifest.get_mut(key).and_then(Value::as_table_mut) else {
    return;
  };
  if let Some(path) = table.get_mut("path") {
    absolutize_string_value(path, crate_dir);
  }
}

fn rewrite_target_source_paths(manifest: &mut Map<String, Value>, key: &str, crate_dir: &Path) {
  let Some(entries) = manifest.get_mut(key).and_then(Value::as_array_mut) else {
    return;
  };
  for entry in entries {
    let Some(table) = entry.as_table_mut() else {
      continue;
    };
    if let Some(path) = table.get_mut("path") {
      absolutize_string_value(path, crate_dir);
    }
  }
}

fn rewrite_dependency_group(
  manifest: &mut Map<String, Value>,
  key: &str,
  crate_dir: &Path,
  config_bridge_dir: &Path,
) {
  let Some(dependencies) = manifest.get_mut(key).and_then(Value::as_table_mut) else {
    return;
  };
  rewrite_dependency_table(dependencies, crate_dir, config_bridge_dir);
}

fn rewrite_target_specific_dependency_groups(
  manifest: &mut Map<String, Value>,
  crate_dir: &Path,
  config_bridge_dir: &Path,
) -> Result<()> {
  let Some(targets) = manifest.get_mut("target").and_then(Value::as_table_mut) else {
    return Ok(());
  };

  for (_, target_table) in targets.iter_mut() {
    let Some(target_table) = target_table.as_table_mut() else {
      bail!("target-specific dependency section is not a table");
    };
    rewrite_dependency_group(target_table, "dependencies", crate_dir, config_bridge_dir);
    rewrite_dependency_group(
      target_table,
      "build-dependencies",
      crate_dir,
      config_bridge_dir,
    );
    rewrite_dependency_group(
      target_table,
      "dev-dependencies",
      crate_dir,
      config_bridge_dir,
    );
  }

  Ok(())
}

fn rewrite_dependency_table(
  dependencies: &mut Map<String, Value>,
  crate_dir: &Path,
  config_bridge_dir: &Path,
) {
  for (dependency_name, dependency_value) in dependencies {
    let Some(table) = dependency_value.as_table_mut() else {
      continue;
    };
    let Some(path) = table.get_mut("path") else {
      continue;
    };
    if dependency_name == GENERATED_CONFIG_PACKAGE {
      *path = Value::String(config_bridge_dir.to_string_lossy().into_owned());
    } else {
      absolutize_string_value(path, crate_dir);
    }
  }
}

fn absolutize_string_value(value: &mut Value, crate_dir: &Path) {
  let Some(path) = value.as_str() else {
    return;
  };
  let path = Path::new(path);
  if path.is_absolute() {
    return;
  }
  *value = Value::String(crate_dir.join(path).to_string_lossy().into_owned());
}

fn harness_signature(
  ctx: &ConfigCliContext,
  config: &ExternalConfig,
  target: ConfigTarget,
  inputs: &HarnessInputs,
) -> String {
  format!(
    "layout={}\nrepo_root={}\nconfig_dir={}\npackage={}\ntarget={}\nrepo_manifest={}\\
     nrepo_lock={}\ntarget_manifest={}\nconfig_manifest={}\n",
    BUILD_LAYOUT_VERSION,
    ctx.repo_root.display(),
    config.dir.display(),
    config.package_name,
    target.as_str(),
    stable_hash_hex(inputs.repo_manifest.as_bytes()),
    inputs
      .repo_lock
      .as_deref()
      .map(|lock| stable_hash_hex(lock.as_bytes()))
      .unwrap_or_else(|| "missing".to_string()),
    stable_hash_hex(inputs.target_manifest.as_bytes()),
    stable_hash_hex(inputs.config_manifest.as_bytes()),
  )
}

fn sync_workspace_lockfile(ctx: &ConfigCliContext, harness: &BuildHarness) -> Result<()> {
  let repo_lock = ctx.repo_root.join("Cargo.lock");
  let harness_lock = harness.root_dir.join("Cargo.lock");
  if repo_lock.exists() {
    fs::copy(&repo_lock, &harness_lock).wrap_err_with(|| {
      format!(
        "failed to copy workspace Cargo.lock from {} to {}",
        repo_lock.display(),
        harness_lock.display()
      )
    })?;
  } else if harness_lock.exists() {
    fs::remove_file(&harness_lock).wrap_err_with(|| {
      format!(
        "failed to remove generated Cargo.lock {}",
        harness_lock.display()
      )
    })?;
  }
  Ok(())
}

fn read_manifest_value(manifest_path: &Path) -> Result<Value> {
  let content = fs::read_to_string(manifest_path)
    .wrap_err_with(|| format!("failed to read {}", manifest_path.display()))?;
  content
    .parse::<Value>()
    .wrap_err_with(|| format!("failed to parse TOML manifest {}", manifest_path.display()))
}

fn read_package_name(manifest_path: &Path) -> Result<String> {
  let package_name = read_manifest_package_name(manifest_path)?;

  if package_name == RESERVED_EXTERNAL_CONFIG_PACKAGE {
    bail!(
      "config crate package name must not be \"{}\"; please rename it in {}",
      RESERVED_EXTERNAL_CONFIG_PACKAGE,
      manifest_path.display()
    );
  }

  Ok(package_name)
}

fn read_manifest_package_name(manifest_path: &Path) -> Result<String> {
  let manifest = read_manifest_value(manifest_path)?;
  manifest
    .get("package")
    .and_then(Value::as_table)
    .and_then(|table| table.get("name"))
    .and_then(Value::as_str)
    .map(|value| value.to_string())
    .ok_or_else(|| {
      eyre!(
        "failed to read package name from {}",
        manifest_path.display()
      )
    })
}

fn write_package_name(manifest_path: &Path, package_name: &str) -> Result<()> {
  if package_name.trim().is_empty() {
    bail!("package name must not be empty");
  }
  if package_name == RESERVED_EXTERNAL_CONFIG_PACKAGE {
    bail!(
      "config crate package name must not be \"{}\"",
      RESERVED_EXTERNAL_CONFIG_PACKAGE
    );
  }

  let mut manifest = read_manifest_value(manifest_path)?;
  let package = manifest
    .get_mut("package")
    .and_then(Value::as_table_mut)
    .ok_or_else(|| eyre!("template manifest is missing [package]"))?;
  package.insert("name".to_string(), Value::String(package_name.to_string()));
  fs::write(
    manifest_path,
    toml::to_string_pretty(&manifest).wrap_err("failed to render config manifest")?,
  )
  .wrap_err_with(|| format!("failed to write {}", manifest_path.display()))
}

fn resolve_config_dir_from_sources(
  override_dir: Option<&Path>,
  default_config_dir: Option<&Path>,
  cwd: &Path,
) -> Result<PathBuf> {
  if let Some(dir) = override_dir {
    return Ok(resolve_cli_path(dir, cwd));
  }

  let Some(default_config_dir) = default_config_dir else {
    bail!("missing default config directory");
  };
  Ok(default_config_dir.to_path_buf())
}

fn resolve_cli_path(path: &Path, cwd: &Path) -> PathBuf {
  if path.is_absolute() {
    path.to_path_buf()
  } else {
    cwd.join(path)
  }
}

fn resolve_output_destination(path: &Path, target: ConfigTarget) -> Result<PathBuf> {
  let cwd = env::current_dir().wrap_err("failed to read current directory")?;
  let resolved = resolve_cli_path(path, &cwd);
  if resolved.is_dir() {
    return Ok(resolved.join(target.binary_file_name()));
  }
  Ok(resolved)
}

fn copy_built_binary(source: &Path, destination: &Path) -> Result<()> {
  if source == destination {
    return Ok(());
  }

  let parent = destination.parent().ok_or_else(|| {
    eyre!(
      "output path does not have a parent directory: {}",
      destination.display()
    )
  })?;
  fs::create_dir_all(parent).wrap_err_with(|| {
    format!(
      "failed to create parent directory for {}",
      destination.display()
    )
  })?;

  if destination.exists() {
    fs::remove_file(destination).wrap_err_with(|| {
      format!(
        "failed to replace existing binary {}",
        destination.display()
      )
    })?;
  }

  fs::copy(source, destination).wrap_err_with(|| {
    format!(
      "failed to copy built binary from {} to {}",
      source.display(),
      destination.display()
    )
  })?;

  Ok(())
}

fn recommended_next_command(
  config_dir: &Path,
  manifest_exists: bool,
  package_name_available: bool,
) -> String {
  if !manifest_exists {
    return format!(
      "the-editor config init --config-dir {}",
      config_dir.display()
    );
  }
  if !package_name_available {
    return format!(
      "fix {} and run the-editor config check",
      config_dir.join("Cargo.toml").display()
    );
  }
  "the-editor config check --target term".to_string()
}

fn copy_dir_all(src: &Path, dst: &Path) -> Result<()> {
  fs::create_dir_all(dst)?;
  for entry in fs::read_dir(src)? {
    let entry = entry?;
    let ty = entry.file_type()?;
    let src_path = entry.path();
    let dst_path = dst.join(entry.file_name());
    if ty.is_dir() {
      copy_dir_all(&src_path, &dst_path)?;
    } else {
      fs::copy(&src_path, &dst_path)?;
    }
  }
  Ok(())
}

fn yes_no(value: bool) -> &'static str {
  if value { "yes" } else { "no" }
}

fn stable_hash_hex(bytes: &[u8]) -> String {
  let mut hash: u64 = 0xCBF29CE484222325;
  for byte in bytes {
    hash ^= u64::from(*byte);
    hash = hash.wrapping_mul(0x100000001B3);
  }
  format!("{hash:016x}")
}

fn normalize_existing_path(path: &Path) -> PathBuf {
  fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

#[cfg(test)]
mod tests {
  use tempfile::tempdir;

  use super::*;

  fn write_manifest(path: &Path, package_name: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(
      path,
      format!("[package]\nname = \"{package_name}\"\nversion = \"0.1.0\"\nedition = \"2024\"\n"),
    )
    .unwrap();
  }

  #[test]
  fn resolve_config_dir_prefers_explicit_override() {
    let cwd = Path::new("/repo");
    let resolved = resolve_config_dir_from_sources(
      Some(Path::new("custom-config")),
      Some(Path::new("/default-config")),
      cwd,
    )
    .unwrap();

    assert_eq!(resolved, PathBuf::from("/repo/custom-config"));
  }

  #[test]
  fn read_package_name_rejects_reserved_name() {
    let temp = tempdir().unwrap();
    let manifest = temp.path().join("Cargo.toml");
    write_manifest(&manifest, "the-config");

    let error = read_package_name(&manifest).unwrap_err().to_string();
    assert!(error.contains("must not be \"the-config\""));
  }

  #[test]
  fn write_package_name_updates_template_manifest() {
    let temp = tempdir().unwrap();
    let manifest = temp.path().join("Cargo.toml");
    write_manifest(&manifest, "the-config-user");

    write_package_name(&manifest, "my-config").unwrap();

    let manifest = fs::read_to_string(&manifest).unwrap();
    assert!(manifest.contains("name = \"my-config\""));
  }

  #[test]
  fn install_path_uses_config_dir_bin() {
    let path = ConfigTarget::Term.install_path(Path::new("/tmp/demo-config"));
    assert_eq!(
      path,
      PathBuf::from("/tmp/demo-config/bin").join(ConfigTarget::Term.binary_file_name())
    );
  }

  #[test]
  fn resolve_output_destination_appends_binary_name_for_directory() {
    let temp = tempdir().unwrap();
    let destination = resolve_output_destination(temp.path(), ConfigTarget::Term).unwrap();
    assert_eq!(
      destination,
      temp.path().join(ConfigTarget::Term.binary_file_name())
    );
  }

  #[test]
  fn rewrite_target_manifest_updates_paths_and_bridge_dependency() {
    let crate_dir = Path::new("/repo/the-term");
    let config_bridge_dir = Path::new("/cache/config-build/hash/the-config");
    let mut manifest = Value::Table(Map::from_iter([
      (
        "package".to_string(),
        Value::Table(Map::from_iter([(
          "build".to_string(),
          Value::String("build.rs".to_string()),
        )])),
      ),
      (
        "bin".to_string(),
        Value::Array(vec![Value::Table(Map::from_iter([(
          "path".to_string(),
          Value::String("main.rs".to_string()),
        )]))]),
      ),
      (
        "dependencies".to_string(),
        Value::Table(Map::from_iter([
          (
            "the-config".to_string(),
            Value::Table(Map::from_iter([(
              "path".to_string(),
              Value::String("../the-config".to_string()),
            )])),
          ),
          (
            "the-default".to_string(),
            Value::Table(Map::from_iter([(
              "path".to_string(),
              Value::String("../the-default".to_string()),
            )])),
          ),
        ])),
      ),
    ]));

    rewrite_target_manifest_paths(
      manifest.as_table_mut().unwrap(),
      crate_dir,
      config_bridge_dir,
    )
    .unwrap();

    let manifest = manifest.as_table().unwrap();
    let package = manifest.get("package").unwrap().as_table().unwrap();
    assert_eq!(
      package.get("build").unwrap().as_str().unwrap(),
      "/repo/the-term/build.rs"
    );
    let bin = manifest.get("bin").unwrap().as_array().unwrap();
    assert_eq!(
      bin[0]
        .as_table()
        .unwrap()
        .get("path")
        .unwrap()
        .as_str()
        .unwrap(),
      "/repo/the-term/main.rs"
    );
    let dependencies = manifest.get("dependencies").unwrap().as_table().unwrap();
    assert_eq!(
      dependencies
        .get("the-config")
        .unwrap()
        .as_table()
        .unwrap()
        .get("path")
        .unwrap()
        .as_str()
        .unwrap(),
      "/cache/config-build/hash/the-config"
    );
    assert_eq!(
      dependencies
        .get("the-default")
        .unwrap()
        .as_table()
        .unwrap()
        .get("path")
        .unwrap()
        .as_str()
        .unwrap(),
      "/repo/the-term/../the-default"
    );
  }

  #[test]
  fn stable_hash_hex_is_deterministic() {
    assert_eq!(
      stable_hash_hex(b"repo\nconfig\nterm\n"),
      stable_hash_hex(b"repo\nconfig\nterm\n")
    );
  }

  #[test]
  fn template_manifest_is_checkout_independent() {
    let content = fs::read_to_string(
      PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("the-config")
        .join("template")
        .join("Cargo.toml"),
    )
    .unwrap();

    assert!(!content.contains("{{THE_EDITOR_REPO}}"));
  }
}
