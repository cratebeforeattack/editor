#[macro_use]
extern crate serde_derive;
extern crate serde;
extern crate serde_json;
//extern crate glob;
extern crate anyhow;
extern crate font_atlas;
extern crate image;
extern crate walkdir;
extern crate zip;

use walkdir::WalkDir;
//use glob::glob;
use anyhow::{Context, Result};
use font_atlas::*;
use std::collections::BTreeMap;
use std::fs;
use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::PathBuf;
use zip::ZipWriter;

#[derive(Serialize, Deserialize)]
struct Glyph {
    uv: [u16; 4],
    pre: [f32; 2],
    post: [f32; 2],
}

#[derive(Serialize, Deserialize)]
struct FontMetrics {
    height: f32,
    ascent: f32,
    descent: f32,
}

#[derive(Serialize, Deserialize)]
struct Font {
    sprite: String,
    char_to_glyph: BTreeMap<String, Glyph>,
    metrics: FontMetrics,
}

fn build_font(
    font_name: &str,
    size: u32,
    desired_w: u32,
    desired_h: u32,
    chars: &[char],
) -> Result<()> {
    let font_filename = format!("{}-{}.font", font_name, size);
    let image_filename = &format!("{}-{}.png", font_name, size);
    if File::open(&font_filename).is_ok() && File::open(&image_filename).is_ok() {
        return Ok(());
    }
    let font = load_font(font_name)?;
    let (atlas, bitmap, height, ascent, descent) = font.make_atlas(
        chars.iter().cloned(),
        size as f32,
        1,
        desired_w as usize,
        desired_h as usize,
    );

    let w = bitmap.width() as u32;
    let h = bitmap.height() as u32;

    let bitmap_rgba = bitmap
        .into_raw()
        .into_iter()
        .map(|x| [255u8, 255, 255, x].to_vec())
        .flatten()
        .collect::<Vec<u8>>();
    image::save_buffer(&image_filename, &bitmap_rgba, w, h, image::ColorType::Rgba8)
        .map_err(|e| Box::new(e))?;

    let runtime_font = {
        let glyphs = chars
            .iter()
            .cloned()
            .filter_map(|ch| atlas.info(ch))
            .map(|ch| {
                (
                    ch.chr.to_string(),
                    Glyph {
                        uv: [
                            ch.bounding_box.x as u16,
                            ch.bounding_box.y as u16,
                            ch.bounding_box.w as u16,
                            ch.bounding_box.h as u16,
                        ],
                        pre: [ch.pre_draw_advance.0, ch.pre_draw_advance.1],
                        post: [ch.post_draw_advance.0, ch.post_draw_advance.1],
                    },
                )
            });
        let mut runtime_font = Font {
            sprite: image_filename.strip_prefix("res/").unwrap().to_owned(),
            char_to_glyph: BTreeMap::new(),
            metrics: FontMetrics {
                height,
                ascent,
                descent,
            },
        };
        for (key, value) in glyphs {
            runtime_font.char_to_glyph.insert(key, value);
        }
        runtime_font
    };

    let content = serde_json::to_vec_pretty(&runtime_font)?;
    BufWriter::new(File::create(&font_filename)?).write_all(&content)?;
    Ok(())
}

fn is_not_hidden(entry: &walkdir::DirEntry) -> bool {
    entry
        .file_name()
        .to_str()
        .map(|s| entry.depth() == 0 || !s.starts_with("."))
        .unwrap_or(false)
}

fn to_unix_path(path: &str) -> String {
    let mut path_str = String::new();
    for component in std::path::Path::new(&path).components() {
        if let std::path::Component::Normal(os_str) = component {
            if !path_str.is_empty() {
                path_str.push('/');
            }
            path_str.push_str(&*os_str.to_string_lossy());
        }
    }
    path_str
}

fn build_res_zip() -> Result<()> {
    let root = "res/";
    let excluded_suffices = [".ttf", ".svg", ".cbmap", ".cbmap.state"];
    let entries: Vec<_> = WalkDir::new(root)
        .into_iter()
        .filter_entry(|e| is_not_hidden(e))
        .filter_map(|v| v.ok())
        .filter(|e| {
            let filename = e.file_name().to_str().unwrap();
            !excluded_suffices.iter().any(|s| filename.ends_with(s))
        })
        .filter(|v| v.file_type().is_file())
        .collect();

    let zip_name = "res.zip";
    let zip_modified = match std::fs::metadata(zip_name) {
        Ok(m) => m.modified().ok(),
        Err(_) => None,
    };
    let mut inputs_changed = false;
    for it in &entries {
        let src_path = it.path();
        println!("cargo:rerun-if-changed={}", src_path.to_str().unwrap());
        if let Some(zip_modified) = zip_modified {
            if std::fs::metadata(src_path)?.modified()? > zip_modified {
                inputs_changed = true;
            }
        } else {
            inputs_changed = true;
        }
    }
    if inputs_changed {
        let mut z = ZipWriter::new(BufWriter::new(File::create(zip_name)?));
        for it in entries {
            let src_path = it.path();
            let dest_path = &it.path().to_str().unwrap()[root.len()..];
            z.start_file(
                &to_unix_path(dest_path),
                zip::write::FileOptions::default()
                    .compression_method(zip::CompressionMethod::Deflated),
            )?;
            let mut content = Vec::new();
            BufReader::new(File::open(src_path)?).read_to_end(&mut content)?;
            z.write_all(&content)?;
        }
        z.finish()?;
        println!("cargo:warning=Updated zip: {}", zip_name);
    }
    Ok(())
}

fn main() -> Result<()> {
    let all_chars = (0x020..=0x07f)
        .into_iter()
        .chain((0x080..=0x0ff).into_iter()) // Latin Suplement
        .chain((0x100..=0x17f).into_iter()) // Latin Extended 1
        .chain((0x400..=0x4ff).into_iter()) // Cyrillic
        .filter_map(|c: u32| std::char::from_u32(c))
        .collect::<Vec<char>>();
    let num_chars = ['0', '1', '2', '3', '4', '5', '6', '7', '8', '9', '.', ':'];

    build_font("res/fonts/BloggerSans.ttf", 16, 64, 64, &all_chars).context("Building font")?;
    build_font("res/fonts/BloggerSans.ttf", 21, 128, 128, &all_chars).context("Building font")?;
    build_font("res/fonts/BloggerSans.ttf", 64, 128, 128, &num_chars).context("Building font")?;

    build_res_zip().context("Building res.zip")?;
    println!("cargo:rerun-if-changed=res.zip");

    let git_hash = {
        if let Err(_) = which::which("git") {
            println!("cargo:warning=Building without git in path.");
            "no-git".to_owned()
        } else {
            // add build dependency on .git/HEAD and ref head it points to
            let git_dir_or_file = PathBuf::from(format!("{}/.git", env!("CARGO_MANIFEST_DIR")));
            if let Ok(metadata) = fs::metadata(&git_dir_or_file) {
                if metadata.is_dir() {
                    let git_head_path = git_dir_or_file.join("HEAD");
                    // Determine where HEAD points and echo that path also.
                    let mut f = File::open(&git_head_path)?;
                    let mut git_head_contents = String::new();
                    let _ = f.read_to_string(&mut git_head_contents)?;
                    let ref_vec: Vec<&str> = git_head_contents.split(": ").collect();
                    println!(
                        "cargo:rerun-if-changed={}/.git/HEAD",
                        env!("CARGO_MANIFEST_DIR")
                    );
                    if ref_vec.len() == 2 {
                        let current_head_file = ref_vec[1];
                        println!(
                            "cargo:rerun-if-changed={}/.git/{}",
                            env!("CARGO_MANIFEST_DIR"),
                            current_head_file
                        );
                    }
                } else {
                    eprintln!(".git is not a directory");
                }
            } else {
                eprintln!("failed to locate .git");
            }

            let output = std::process::Command::new("git")
                .args(&["rev-parse", "--short", "HEAD"])
                .output()
                .context("Invoking Git")?;
            String::from_utf8(output.stdout).unwrap()
        }
    };
    println!("cargo:rustc-env=GIT_HASH={}", git_hash);

    Ok(())
}
