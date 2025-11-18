use std::fs::File;
use std::path::Path;
use std::env;

use flate2::write::GzEncoder;
use flate2::Compression;
use tar::Builder;
use walkdir::WalkDir;
use zip::{write::FileOptions, ZipWriter};

// Make sure sbatchman_configs is public in core/mod.rs
use crate::core::sbatchman_configs::get_sbatchman_dir;

/// Export the .sbatchman directory into either "zip" or "tar.gz"
/// Default is "tar.gz" if `format` is None or invalid.
pub fn export(format: Option<&str>, compressed_filename: Option<&str>) {
    // Determine format
    let format = match format {
        Some("zip") => "zip",
        _ => "tar.gz", // default
    };

    let filename = match compressed_filename {
        Some(name) => name.to_string(),
        _ => String::from("sbatchman"),
    };

    // Locate .sbatchman directory
    let sbatch_dir = match get_sbatchman_dir() {
        Ok(path) => path,
        Err(e) => {
            eprintln!("❌ Could not find .sbatchman directory: {:?}", e);
            return;
        }
    };

    let config = match crate::core::sbatchman_configs::get_sbatchman_config(&sbatch_dir) {
        Ok(cfg) => cfg,
        Err(e) => {
            eprintln!("❌ Could not read sbatchman config: {:?}", e);
            return;
        }
    };

    let clustername = match &config.cluster_name {
        Some(name) => name,
        None => {
            eprintln!("❌ Cluster name not found in sbatchman config");
            return;
        }
    };

    println!("✅ Found .sbatchman at: {}", sbatch_dir.display());

    let ts = chrono::Local::now().format("%Y%m%d_%H%M%S").to_string(); 

    let out_name = format!("{}_{}_{}_.{}", filename, clustername, ts, format);
    let out_path = match env::home_dir() {
        Some(cd) => cd.join(&out_name),
        None => {
            eprintln!("❌ Could not determine home directory");
            return;
        }
    };

    println!("📦 Exporting .sbatchman as {} → {}", format, out_path.display());

    // Compress
    let result = if format == "zip" {
        create_zip(&sbatch_dir, &out_path)
    } else {
        create_tar_gz(&sbatch_dir, &out_path)
    };

    match result {
        Ok(_) => println!("✅ Archive created successfully!"),
        Err(e) => eprintln!("❌ Failed to create archive: {}", e),
    }
}

// ---- ZIP creation ----
fn create_zip(src_dir: &Path, dest_file: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let file = File::create(dest_file)?;
    let mut zip = ZipWriter::new(file);
    let options: FileOptions<'_, ()> = FileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    for entry in WalkDir::new(src_dir) {
        let entry = entry.map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;
        let path = entry.path();
        let name = path.strip_prefix(src_dir).unwrap();

        if path.is_file() {
            zip.start_file(name.to_string_lossy(), options)?;
            let mut f = File::open(path)?;
            std::io::copy(&mut f, &mut zip)?;
        } else if !name.as_os_str().is_empty() {
            zip.add_directory(name.to_string_lossy(), options)?;
        }
    }

    zip.finish()?;
    Ok(())
}

// ---- TAR.GZ creation ----
fn create_tar_gz(src_dir: &Path, dest_file: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let tar_gz = File::create(dest_file)?;
    let enc = GzEncoder::new(tar_gz, Compression::default());
    let mut tar = Builder::new(enc);
    
    // Use the directory name instead of "." to preserve structure
    let dir_name = src_dir.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(".sbatchman");
    
    tar.append_dir_all(dir_name, src_dir)?;
    Ok(())
}