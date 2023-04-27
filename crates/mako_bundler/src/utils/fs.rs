use rsfs::*;
use std::io;
use std::io::prelude::*;
use std::path::Path;

#[cfg(not(feature = "memory-fs"))]
pub use rsfs::disk::FS;
#[cfg(feature = "memory-fs")]
pub use rsfs::mem::FS;

#[cfg(not(feature = "memory-fs"))]
pub use rsfs::disk::File;
#[cfg(feature = "memory-fs")]
pub use rsfs::mem::File;

pub fn read_to_string<P: AsRef<Path>>(path: P) -> io::Result<String> {
    let mut file = FS.open_file(path)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    Ok(contents)
}

pub fn write<P: AsRef<Path>, C: AsRef<[u8]>>(path: P, contents: C) -> io::Result<()> {
    fn inner(path: &Path, contents: &[u8]) -> io::Result<()> {
        let f = FS {};
        FS::create_file(&f, path)?.write_all(contents)
    }
    inner(path.as_ref(), contents.as_ref())
}

pub fn read<P: AsRef<Path>>(path: P) -> io::Result<Vec<u8>> {
    fn inner(path: &Path) -> io::Result<Vec<u8>> {
        let f = FS {};
        let mut file = FS::open_file(&f, path)?;
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)?;
        Ok(bytes)
    }
    inner(path.as_ref())
}
