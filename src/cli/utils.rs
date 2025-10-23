use anyhow::{Context, Result};
use sha2::{Digest, Sha256};
use std::io::Read;
use std::{env, fs, os::unix::fs::PermissionsExt, process::Command};

pub fn update() -> Result<()> {
  let repo = "ThomasPasquali/sbatchman";
  let (os, arch) = (env::consts::OS, env::consts::ARCH);
  let target = match (os, arch) {
    ("linux", "x86_64") => "x86_64-unknown-linux-musl",
    ("linux", "aarch64") => "aarch64-unknown-linux-musl",
    ("macos", "x86_64") => "x86_64-apple-darwin",
    ("macos", "aarch64") => "aarch64-apple-darwin",
    _ => return Err(anyhow::anyhow!("Unsupported platform")),
  };

  // Fetch release info
  let output = Command::new("curl")
    .args([
      "-fsSL",
      &format!("https://api.github.com/repos/{repo}/releases/latest"),
    ])
    .output()
    .context("Failed to fetch release info")?;

  let json = String::from_utf8(output.stdout)?;
  let bin_url = json
    .lines()
    .find(|l| l.contains(&format!("sbatchman-{target}\"")))
    .and_then(|l| l.split('"').nth(3))
    .context("No binary found")?;
  let sha_url = json
    .lines()
    .find(|l| l.contains(&format!("sbatchman-{target}.sha256")))
    .and_then(|l| l.split('"').nth(3))
    .context("No checksum found")?;

  let tmp_bin = "/tmp/sbatchman-new";
  let tmp_sha = "/tmp/sbatchman.sha256";

  // Download both
  Command::new("curl")
    .args(["-fsSL", bin_url, "-o", tmp_bin])
    .status()?;
  Command::new("curl")
    .args(["-fsSL", sha_url, "-o", tmp_sha])
    .status()?;

  // Verify checksum
  let mut sha_file = std::fs::File::open(tmp_sha)?;
  let mut expected_hash = String::new();
  sha_file.read_to_string(&mut expected_hash)?;
  let expected_hash = expected_hash.split_whitespace().next().unwrap_or("").trim();

  let mut file = std::fs::File::open(tmp_bin)?;
  let mut hasher = Sha256::new();
  std::io::copy(&mut file, &mut hasher)?;
  let actual_hash = format!("{:x}", hasher.finalize());

  if actual_hash != expected_hash {
    return Err(anyhow::anyhow!("Checksum verification failed!"));
  }

  // Replace current binary
  fs::set_permissions(tmp_bin, fs::Permissions::from_mode(0o755))?;
  let current_exe = env::current_exe()?;
  fs::rename(tmp_bin, current_exe)?;

  println!("✅ sbatchman updated and verified successfully!");
  Ok(())
}
