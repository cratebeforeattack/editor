use std::cell::RefCell;
use std::io::Read;
use zip::ZipArchive;
use anyhow::{Result, Error, bail};

enum Mount {
    ZipStatic {
        name: String,
        archive: zip::ZipArchive<std::io::Cursor<&'static [u8]>>,
    },
}

struct Root {
    mounts: Vec<Mount>,
}

thread_local! {
    static FILESYSTEM_ROOT: RefCell<Root> = {
        let resources_zip = include_bytes!("../res.zip");
        let slice = &resources_zip[..];
        RefCell::new(Root{
            mounts: vec![
                Mount::ZipStatic{
                    name: "res.zip".to_owned(),
                    archive: ZipArchive::new(std::io::Cursor::new(slice)).expect("broken zip")
                }
            ]
        })
    };
}


pub fn load_file(filename: &str) -> Result<Vec<u8>> {
    FILESYSTEM_ROOT.with(|root| -> Result<Vec<u8>> {
        let mut root = root.borrow_mut();
        for mount in root.mounts.iter_mut() {
            match mount {
                Mount::ZipStatic { name: _name, archive } => {
                    let zip = archive;
                    let mut f = match zip.by_name(filename) {
                        Ok(f) => f,
                        _ => continue,
                    };
                    let mut content = Vec::new();
                    f.read_to_end(&mut content)?;
                    return Ok(content);
                }
            }
        }
        bail!("File not found in ZIP: {}", filename);
    })
}
