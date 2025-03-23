use std::ffi::CString;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

use tokio::fs;
use tokio_retry::Retry;

use super::logger::{log_verbose, log_warning};
use super::retry::create_retry_strategy;
#[cfg(target_os = "macos")]
use libc::clonefile;
#[cfg(target_os = "linux")]
use {
    libc::{ioctl, FICLONE},
    std::os::unix::io::AsRawFd,
};


pub async fn validate_directory(src: &Path, dst: &Path) -> std::io::Result<bool> {
    if !fs::metadata(src).await?.is_dir() || !fs::metadata(dst).await?.is_dir() {
        log_verbose("validating failed, since it's not a directory");
        return Ok(false);
    }

    #[derive(Debug)]
    struct EntryInfo {
        path: PathBuf,
        is_dir: bool,
        size: u64,
    }

    async fn collect_entries(
        dir: &Path,
        ignore: Option<&[&str]>,
    ) -> std::io::Result<Vec<EntryInfo>> {
        let mut entries = Vec::new();
        let mut read_dir = fs::read_dir(dir).await?;

        while let Some(entry) = read_dir.next_entry().await? {
            if let Some(ignore_list) = ignore {
                if let Some(file_name) = entry.path().file_name() {
                    if ignore_list.contains(&file_name.to_str().unwrap_or_default()) {
                        continue;
                    }
                }
            }

            let metadata = entry.metadata().await?;
            entries.push(EntryInfo {
                path: entry.path(),
                is_dir: metadata.is_dir(),
                size: if metadata.is_file() {
                    metadata.len()
                } else {
                    0
                },
            });
        }
        Ok(entries)
    }

    let mut src_entries = collect_entries(src, Some(&["node_modules"])).await?;
    let mut dst_entries = collect_entries(dst, Some(&["node_modules"])).await?;

    src_entries.sort_by_key(|e| e.path.clone());
    dst_entries.sort_by_key(|e| e.path.clone());

    if src_entries.len() != dst_entries.len() {
        log_verbose(&format!("validating failed {}:{} to {}:{}, since entries length is not equal\nsrc entries: {:?}\ndst entries: {:?}",
            src.display(), src_entries.len(), dst.display(), dst_entries.len(),
            src_entries.iter().map(|e| e.path.file_name().unwrap_or_default()).collect::<Vec<_>>(),
            dst_entries.iter().map(|e| e.path.file_name().unwrap_or_default()).collect::<Vec<_>>()));
        return Ok(false);
    }

    for (src_entry, dst_entry) in src_entries.iter().zip(dst_entries.iter()) {
        if src_entry.is_dir && dst_entry.is_dir {
            if !Box::pin(validate_directory(&src_entry.path, &dst_entry.path)).await? {
                return Ok(false);
            }
        } else if !src_entry.is_dir && !dst_entry.is_dir {
            if src_entry.size != dst_entry.size {
                log_verbose(&format!(
                    "validating failed {}:{} to {}:{}, since diff size",
                    src_entry.path.display(),
                    src_entry.size,
                    dst_entry.path.display(),
                    dst_entry.size
                ));
                return Ok(false);
            }
        } else {
            log_verbose(&format!(
                "validating failed {}:{} to {}:{}, since diff size",
                src_entry.path.display(),
                src_entry.size,
                dst_entry.path.display(),
                dst_entry.size
            ));
            return Ok(false);
        }
    }

    Ok(true)
}

// find the first non builded subdirectory
pub async fn find_real_src<P: AsRef<Path>>(src: P) -> Option<PathBuf> {
    // 查找第一个非 builded 的子目录
    let mut read_dir = fs::read_dir(src.as_ref()).await.ok()?;
    while let Some(entry) = read_dir.next_entry().await.ok()? {
        if let Ok(metadata) = entry.metadata().await {
            if metadata.is_dir() {
                if let Some(name) = entry.path().file_name() {
                    if name.to_str().unwrap_or_default() != ".utoo_builded" {
                        return Some(entry.path());
                    }
                }
            }
        }
    }
    None
}

pub async fn clone(src: &Path, dst: &Path, find_real: bool) -> Result<(), std::io::Error> {
    let real_src = if find_real {
        find_real_src(src).await.ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("Cannot find valid source directory in {:?}", src),
            )
        })?
    } else {
        src.to_path_buf()
    };

    if find_real && fs::metadata(dst).await.is_ok() {
        if validate_directory(&real_src, dst).await? {
            log_verbose(&format!(
                "Target directory {} already exists and validation passed, skipping clone",
                dst.display()
            ));
            return Ok(());
        } else {
            log_warning(&format!("{:?} --> {:?} overrides", real_src, dst));
            if let Err(e) = fs::remove_dir_all(dst).await {
                log_warning(&format!(
                    "Failed to clean target directory {}: {}",
                    dst.display(),
                    e
                ));
            }
        }
    }

    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent).await?;
    }
    #[cfg(target_os = "macos")]
    {
        let src_c = CString::new(real_src.as_os_str().as_bytes())?;
        let dst_c = CString::new(dst.as_os_str().as_bytes())?;

        Retry::spawn(create_retry_strategy(), || async {
            match unsafe { clonefile(src_c.as_ptr(), dst_c.as_ptr(), 0) } {
                0 => {
                    log_verbose(&format!(
                        "clone {} to {} success",
                        real_src.display(),
                        dst.display()
                    ));
                    Ok(())
                }
                _ => {
                    let _ = fs::remove_dir_all(dst).await;
                    Err(std::io::Error::last_os_error())
                }
            }
        })
        .await
    }

    #[cfg(target_os = "linux")]
    {
        Retry::spawn(create_retry_strategy(), || async {
            match clone_with_cow(&real_src, dst).await {
                Ok(()) => {
                    log_verbose(&format!(
                        "clone {} to {} success",
                        real_src.display(),
                        dst.display()
                    ));
                    Ok(())
                }
                Err(e) => {
                    let _ = fs::remove_dir_all(dst).await;
                    Err(e)
                }
            }
        })
        .await
    }
}

#[cfg(target_os = "linux")]
async fn clone_with_cow(src: &Path, dst: &Path) -> std::io::Result<()> {
    if fs::metadata(src).await?.is_dir() {
        fs::create_dir_all(dst).await?;
        let mut read_dir = fs::read_dir(src).await?;
        while let Some(entry) = read_dir.next_entry().await? {
            let dst_path = dst.join(entry.file_name());
            clone_with_cow(&entry.path(), &dst_path).await?;
        }
        Ok(())
    } else {
        // 尝试使用 FICLONE（如果文件系统支持）
        let src_file = std::fs::File::open(src)?;
        let dst_file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(dst)?;

        // 先尝试 FICLONE
        let result = unsafe {
            ioctl(dst_file.as_raw_fd(), FICLONE as _, src_file.as_raw_fd())
        };

        if result == 0 {
            Ok(())
        } else {
            // 如果 FICLONE 失败，回退到 copy_file_range
            let mut offset: i64 = 0;
            let len = src_file.metadata()?.len();

            while offset < len as i64 {
                let result = unsafe {
                    libc::copy_file_range(
                        src_file.as_raw_fd(),
                        &mut offset as *mut i64,
                        dst_file.as_raw_fd(),
                        &mut offset as *mut i64,
                        (len - offset as u64) as usize,
                        0,
                    )
                };

                if result < 0 {
                    return Err(std::io::Error::last_os_error());
                }
            }
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use tokio::io::AsyncWriteExt;

    async fn create_test_file(dir: &Path, name: &str, content: &[u8]) -> std::io::Result<PathBuf> {
        let path = dir.join(name);
        let mut file = fs::File::create(&path).await?;
        file.write_all(content).await?;
        Ok(path)
    }

    async fn create_test_structure(
        dir: &Path,
        structure: &[(&str, Option<&[u8]>)],
    ) -> std::io::Result<()> {
        for (path, content) in structure {
            let full_path = dir.join(path);
            if let Some(content) = content {
                if let Some(parent) = full_path.parent() {
                    fs::create_dir_all(parent).await?;
                }
                let mut file = fs::File::create(&full_path).await?;
                file.write_all(content).await?;
            } else {
                fs::create_dir_all(full_path).await?;
            }
        }
        Ok(())
    }

    #[tokio::test]
    async fn test_validate_directory_different_sizes() -> std::io::Result<()> {
        let temp = TempDir::new()?;
        let src_dir = temp.path().join("src");
        let dst_dir = temp.path().join("dst");

        create_test_structure(&src_dir, &[("file.txt", Some(b"content1"))]).await?;
        create_test_structure(&dst_dir, &[("file.txt", Some(b"different"))]).await?;

        assert!(!validate_directory(&src_dir, &dst_dir).await?);
        Ok(())
    }

    #[tokio::test]
    async fn test_find_real_src() -> std::io::Result<()> {
        let temp = TempDir::new()?;
        let dir = temp.path().join("test_dir");
        fs::create_dir(&dir).await?;

        assert!(find_real_src(&dir).await.is_none());

        create_test_file(&dir, "file.txt", b"content").await?;
        assert!(find_real_src(&dir).await.is_none());

        let subdir = dir.join("subdir");
        fs::create_dir(&subdir).await?;
        assert_eq!(find_real_src(&dir).await.unwrap(), subdir);

        Ok(())
    }
}
