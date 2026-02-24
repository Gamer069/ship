use std::{
    collections::{HashMap, HashSet},
    io::{Cursor, Error, ErrorKind, Read, Write},
    path::{Path, PathBuf},
};

use deb::{DebFile, binary::DebPackage};

use crate::{conf::ShipConfig, gen_::Generator};

pub struct DebGenerator<'a> {
    pub conf: &'a ShipConfig,
}

impl<'a> DebGenerator<'a> {
    pub fn new(conf: &'a ShipConfig) -> Self {
        Self { conf }
    }
}

impl<'a> Generator for DebGenerator<'a> {
    fn run(&self) {
        let files = self
            .conf
            .files
            .paths
            .iter()
            .map(|file| {
                let to = format!(
                    "/opt/{}/{}",
                    self.conf.prog.name,
                    file.strip_prefix("./").unwrap_or(file)
                );

                (file.clone(), to)
            })
            .collect::<Vec<(String, String)>>();

        let mut pkg = DebPackage::new(&self.conf.prog.name);
        let mut bin_symlinks: Vec<(String, String)> = Vec::new();
        let mut seen_links: HashMap<String, String> = HashMap::new();

        for (from, to) in &files {
            if let Some(link_name) = executable_name(from) {
                let link_path = format!("/usr/bin/{link_name}");

                if let Some(existing_target) = seen_links.get(&link_path) {
                    if existing_target != to {
                        eprintln!(
                            "error: conflicting binaries for {link_path}: {} and {}",
                            existing_target, to
                        );
                        return;
                    }
                    continue;
                }

                seen_links.insert(link_path.clone(), to.clone());
                bin_symlinks.push((link_path, to.clone()));
            }
        }

        for (from, to) in files {
            let from_path = Path::new(&from);

            if from_path.is_dir() {
                pkg = add_dir_recursive(pkg, from_path, Path::new(&to));
            } else {
                let file = match DebFile::from_path(from, to) {
                    Ok(f) => f,
                    Err(err) => {
                        eprintln!("error: failed to generate .deb! {err}");
                        return; // exits run(), not just the closure
                    }
                };
                pkg = pkg.with_file(file);
            }
        }

        pkg = pkg
            .set_name(&self.conf.prog.name)
            .set_maintainer(&self.conf.prog.author)
            .set_architecture(self.conf.prog.arch.deb());

        if let Some(ref version) = self.conf.prog.version {
            pkg = pkg.set_version(&version);
        }

        let output_path = self.deb_output_path();
        if let Some(parent) = output_path.parent() {
            std::fs::create_dir_all(parent).unwrap_or_else(|err| {
                eprintln!(
                    "error: failed to create output directory {}: {err}",
                    parent.display()
                );
                std::process::exit(-1);
            });
        }

        let archive = pkg.build().unwrap_or_else(|err| {
            eprintln!("error: failed to build .deb package: {err}");
            std::process::exit(-1);
        });

        let mut deb_bytes = Vec::new();
        archive.write(&mut deb_bytes).unwrap_or_else(|err| {
            eprintln!("error: failed to serialize .deb package: {err}");
            std::process::exit(-1);
        });

        if !bin_symlinks.is_empty() {
            deb_bytes = rewrite_deb_with_symlinks(&deb_bytes, &bin_symlinks).unwrap_or_else(
                |err| {
                    eprintln!("error: failed to add symlinks to .deb data archive: {err}");
                    std::process::exit(-1);
                },
            );
        }

        std::fs::write(&output_path, deb_bytes).unwrap_or_else(|err| {
            eprintln!(
                "error: failed to write .deb package at {}: {err}",
                output_path.display()
            );
            std::process::exit(-1);
        });
    }
}

impl<'a> DebGenerator<'a> {
    fn deb_output_path(&self) -> PathBuf {
        let out = Path::new(&self.conf.out.bin);
        if out.extension().and_then(|ext| ext.to_str()) == Some("deb") {
            return out.to_path_buf();
        }

        let mut file_name = self.conf.prog.name.clone();
        if let Some(version) = &self.conf.prog.version {
            file_name.push('_');
            file_name.push_str(version);
        }
        file_name.push('_');
        file_name.push_str(&format!("{:?}", self.conf.prog.arch).to_lowercase());
        file_name.push_str(".deb");

        out.join(file_name)
    }
}

#[cfg(unix)]
fn executable_name(path: &str) -> Option<String> {
    use std::os::unix::fs::PermissionsExt;

    let path = Path::new(path);
    let metadata = std::fs::metadata(path).ok()?;
    if !metadata.is_file() || metadata.permissions().mode() & 0o111 == 0 {
        return None;
    }
    path.file_name()
        .and_then(|name| name.to_str())
        .map(|name| name.to_string())
}

#[cfg(not(unix))]
fn executable_name(_path: &str) -> Option<String> {
    None
}

enum DataCompression {
    Xz,
    Zstd,
}

fn rewrite_deb_with_symlinks(
    deb_bytes: &[u8],
    bin_symlinks: &[(String, String)],
) -> std::io::Result<Vec<u8>> {
    let mut archive = ar::Archive::new(Cursor::new(deb_bytes));
    let mut entries: Vec<(Vec<u8>, u32, Vec<u8>)> = Vec::new();

    while let Some(entry_result) = archive.next_entry() {
        let mut entry = entry_result?;
        let identifier = entry.header().identifier().to_vec();
        let mode = entry.header().mode();
        let mut contents = Vec::new();
        entry.read_to_end(&mut contents)?;
        entries.push((identifier, mode, contents));
    }

    let data_index = entries
        .iter()
        .position(|(identifier, _, _)| {
            let name = ar_identifier_to_name(identifier);
            name == "data.tar.zst" || name == "data.tar.xz"
        })
        .ok_or_else(|| Error::new(ErrorKind::Other, "deb package missing data archive"))?;

    let data_name = ar_identifier_to_name(&entries[data_index].0);
    entries[data_index].2 = rewrite_data_archive(&entries[data_index].2, &data_name, bin_symlinks)?;

    let mut output = Vec::new();
    let mut builder = ar::Builder::new(&mut output);
    for (identifier, mode, contents) in entries {
        let mut header = ar::Header::new(identifier, contents.len().try_into().unwrap());
        header.set_mode(mode);
        builder.append(&header, contents.as_slice())?;
    }
    drop(builder);

    Ok(output)
}

fn rewrite_data_archive(
    data_archive: &[u8],
    data_name: &str,
    bin_symlinks: &[(String, String)],
) -> std::io::Result<Vec<u8>> {
    let compression = if data_name.ends_with(".zst") {
        DataCompression::Zstd
    } else if data_name.ends_with(".xz") {
        DataCompression::Xz
    } else {
        return Err(Error::new(
            ErrorKind::Other,
            format!("unsupported data archive format: {data_name}"),
        ));
    };

    let mut tar_buf = Vec::new();
    match compression {
        DataCompression::Zstd => {
            zstd::stream::copy_decode(Cursor::new(data_archive), &mut tar_buf)?;
        }
        DataCompression::Xz => {
            xz2::read::XzDecoder::new(Cursor::new(data_archive)).read_to_end(&mut tar_buf)?;
        }
    }

    let mut old_tar = tar::Archive::new(Cursor::new(tar_buf));
    let mut new_tar = tar::Builder::new(Vec::new());
    let mut existing_paths = HashSet::new();

    for entry_result in old_tar.entries()? {
        let mut entry = entry_result?;
        let entry_path = entry.path()?.into_owned();
        existing_paths.insert(entry_path.to_string_lossy().into_owned());

        let entry_type = entry.header().entry_type();
        let mode = entry.header().mode()?;
        let mut contents = Vec::new();
        entry.read_to_end(&mut contents)?;

        let mut header = tar::Header::new_gnu();
        header.set_path(&entry_path)?;
        header.set_mode(mode);
        header.set_entry_type(entry_type);
        if entry_type.is_symlink() || entry_type.is_hard_link() {
            if let Some(link_name) = entry.link_name()? {
                header.set_link_name(link_name.as_ref())?;
            }
        }
        header.set_size(contents.len().try_into().unwrap());
        header.set_cksum();
        new_tar.append(&header, contents.as_slice())?;
    }

    for (link, target) in bin_symlinks {
        let link_path = link.strip_prefix('/').unwrap_or(link);
        if existing_paths.contains(link_path) {
            return Err(Error::new(
                ErrorKind::Other,
                format!("data archive already contains path: {link_path}"),
            ));
        }

        let mut header = tar::Header::new_gnu();
        header.set_path(link_path)?;
        header.set_entry_type(tar::EntryType::symlink());
        header.set_link_name(target)?;
        header.set_mode(0o777);
        header.set_size(0);
        header.set_cksum();
        new_tar.append(&header, std::io::empty())?;
    }

    let new_tar_buf = new_tar.into_inner()?;
    let mut output = Vec::new();
    match compression {
        DataCompression::Zstd => {
            zstd::stream::copy_encode(Cursor::new(new_tar_buf), &mut output, 0)?;
        }
        DataCompression::Xz => {
            let mut encoder = xz2::write::XzEncoder::new(Vec::new(), 9);
            encoder.write_all(&new_tar_buf)?;
            output = encoder.finish()?;
        }
    }

    Ok(output)
}

fn ar_identifier_to_name(identifier: &[u8]) -> String {
    let mut name = String::from_utf8_lossy(identifier).into_owned();
    while name.ends_with(' ') {
        name.pop();
    }
    if let Some(stripped) = name.strip_suffix('/') {
        stripped.to_string()
    } else {
        name
    }
}

// helper function to recursively add a directory to the package
fn add_dir_recursive(mut pkg: DebPackage, from: &Path, to: &Path) -> DebPackage {
    for entry in std::fs::read_dir(from).unwrap_or_else(|err| {
        eprintln!("error: failed to read directory {from:?}! {err}");
        std::process::exit(-1);
    }) {
        let entry = entry.unwrap_or_else(|err| {
            eprintln!("error: failed to read directory entry in {from:?}! {err}");
            std::process::exit(-1);
        });

        let path = entry.path();
        let target_path = to.join(entry.file_name());

        if path.is_file() {
            let file = match DebFile::from_path(&path, &target_path) {
                Ok(f) => f,
                Err(err) => {
                    eprintln!("error: failed to generate .deb! {err}");
                    std::process::exit(-1);
                }
            };
            pkg = pkg.with_file(file);
        } else if path.is_dir() {
            pkg = add_dir_recursive(pkg, &path, &target_path);
        }
    }
    pkg
}
