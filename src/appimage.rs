use std::path::{Path, PathBuf};

use appimage::AppImage;

use crate::{conf::ShipConfig, gen_::Generator};

pub struct AppImageGenerator<'a> {
    pub conf: &'a ShipConfig,
}

impl<'a> AppImageGenerator<'a> {
    pub fn new(conf: &'a ShipConfig) -> Self {
        Self { conf }
    }

    fn appimage_output_path(&self) -> PathBuf {
        let out = PathBuf::from(&self.conf.out.bin);
        if out.extension().and_then(|ext| ext.to_str()) == Some("AppImage") {
            return out;
        }

        if out.is_dir() || self.conf.out.bin.ends_with('/') {
            let mut file_name = self.conf.prog.name.clone();
            if let Some(version) = &self.conf.prog.version {
                file_name.push('_');
                file_name.push_str(version);
            }
            file_name.push('_');
            file_name.push_str(&format!("{:?}", self.conf.prog.arch).to_lowercase());
            file_name.push_str(".AppImage");
            return out.join(file_name);
        }

        out
    }
}

impl<'a> Generator for AppImageGenerator<'a> {
    fn run(&self) {
        let output_path = self.appimage_output_path();
        let build_dir = output_path.parent().unwrap_or_else(|| Path::new("."));

        std::fs::create_dir_all(build_dir).unwrap_or_else(|err| {
            eprintln!(
                "error: failed to create output directory {}: {err}",
                build_dir.display()
            );
            std::process::exit(-1);
        });

        let image = AppImage::new(build_dir, self.conf.prog.name.clone()).unwrap_or_else(|err| {
            eprintln!("error: failed to initialize AppImage build directory: {err}");
            std::process::exit(-1);
        });

        let primary = self
            .conf
            .files
            .paths
            .iter()
            .find(|path| {
                let p = Path::new(path);
                p.is_file()
                    && p.file_name().and_then(|n| n.to_str()) == Some(self.conf.prog.name.as_str())
            })
            .or_else(|| {
                self.conf
                    .files
                    .paths
                    .iter()
                    .find(|path| Path::new(path).is_file())
            });

        if let Some(primary) = primary {
            image
                .add_file(Path::new(primary), Path::new(&self.conf.prog.name))
                .unwrap_or_else(|err| {
                    eprintln!("error: failed to add main executable {primary} to AppImage: {err}");
                    std::process::exit(-1);
                });
        } else {
            eprintln!("error: no file entries found in [files].paths for AppImage target");
            std::process::exit(-1);
        }

        for file in &self.conf.files.paths {
            let from = Path::new(file);
            let fname = match from.file_name() {
                Some(name) => name,
                None => {
                    eprintln!("error: invalid path in [files].paths: {file}");
                    std::process::exit(-1);
                }
            };

            let to = Path::new("usr").join("bin").join(fname);

            if from.is_dir() {
                image.add_directory(from, &to).unwrap_or_else(|err| {
                    eprintln!("error: failed to add directory {:?} to AppImage: {err}", from);
                    std::process::exit(-1);
                });
            } else {
                image.add_file(from, &to).unwrap_or_else(|err| {
                    eprintln!("error: failed to add file {:?} to AppImage: {err}", from);
                    std::process::exit(-1);
                });
            }
        }

        image.add_apprun().unwrap_or_else(|err| {
            eprintln!("error: failed to create AppRun symlink: {err}");
            std::process::exit(-1);
        });

        image.add_desktop().unwrap_or_else(|err| {
            eprintln!("error: failed to generate desktop entry: {err}");
            std::process::exit(-1);
        });

        let generated_icon_path = if let Some(icon) = &self.conf.files.icon {
            image.add_icon(Path::new(icon)).unwrap_or_else(|err| {
                eprintln!("error: failed to add icon {icon}: {err}");
                std::process::exit(-1);
            });
            None
        } else {
            let fallback = std::env::temp_dir().join(format!(
                "{}-{}-fallback-icon.svg",
                self.conf.prog.name,
                std::process::id()
            ));
            std::fs::write(&fallback, fallback_icon_svg(&self.conf.prog.name)).unwrap_or_else(
                |err| {
                    eprintln!(
                        "error: failed to generate fallback icon at {}: {err}",
                        fallback.display()
                    );
                    std::process::exit(-1);
                },
            );
            image.add_icon(&fallback).unwrap_or_else(|err| {
                eprintln!("error: failed to add fallback icon: {err}");
                std::process::exit(-1);
            });
            Some(fallback)
        };

        image.build(&output_path, None).unwrap_or_else(|err| {
            eprintln!(
                "error: failed to build AppImage at {}: {err}",
                output_path.display()
            );
            std::process::exit(-1);
        });

        if let Some(path) = generated_icon_path {
            std::fs::remove_file(path).ok();
        }
    }
}

fn fallback_icon_svg(app_name: &str) -> String {
    let initial = app_name
        .chars()
        .find(|c| c.is_ascii_alphanumeric())
        .unwrap_or('S')
        .to_ascii_uppercase();
    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"256\" height=\"256\" viewBox=\"0 0 256 256\">\
         <rect width=\"256\" height=\"256\" rx=\"36\" fill=\"#1f2937\"/>\
         <text x=\"50%\" y=\"56%\" dominant-baseline=\"middle\" text-anchor=\"middle\" \
         font-family=\"sans-serif\" font-size=\"120\" fill=\"#f9fafb\">{initial}</text>\
         </svg>"
    )
}
