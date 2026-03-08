#[derive(Clone)]
pub(super) struct FileMeta {
    pub mmap: ::std::sync::Arc<memmap2::Mmap>,
    pub length: usize,
    pub modified: ::std::time::SystemTime,
    pub etag: ksbh_types::KsbhStr,
    pub mime: ksbh_types::KsbhStr,
}

pub(super) struct FileCache {
    pub files: ::std::sync::Arc<
        tokio::sync::RwLock<::std::collections::HashMap<::std::path::PathBuf, FileMeta>>,
    >,
}

impl FileCache {
    pub fn new() -> Self {
        Self {
            files: ::std::sync::Arc::new(tokio::sync::RwLock::new(
                ::std::collections::HashMap::new(),
            )),
        }
    }

    pub async fn get(&self, path: &::std::path::Path) -> Option<FileMeta> {
        let meta = ::std::fs::metadata(path).ok()?;
        let modified = meta.modified().ok()?;
        let length = meta.len() as usize;

        let mut map = self.files.write().await;
        if let Some(cached) = map.get(path)
            && cached.modified == modified
        {
            return Some(cached.clone());
        }

        let file = ::std::fs::File::open(path).ok()?;
        let mmap = unsafe { memmap2::Mmap::map(&file).ok()? };
        let mmap = ::std::sync::Arc::new(mmap);

        let etag = ksbh_types::KsbhStr::new(format!(
            "\"{:x}-{:x}\"",
            modified
                .duration_since(::std::time::UNIX_EPOCH)
                .ok()?
                .as_secs(),
            length
        ));

        let new_meta = FileMeta {
            mmap,
            length,
            modified,
            etag,
            mime: ksbh_types::KsbhStr::new(
                mime_guess::from_path(path)
                    .first_or_octet_stream()
                    .essence_str(),
            ),
        };
        map.insert(path.to_path_buf(), new_meta.clone());
        Some(new_meta)
    }
}
