use std::cell::RefCell;
use std::fs::File;
use std::num::NonZeroUsize;
use std::os::unix::io::AsRawFd;
use std::path::Path;
#[cfg(feature = "lru")]
use std::path::PathBuf;
use std::rc::Rc;

#[cfg(feature = "lru")]
use lru::LruCache;

#[cfg(feature = "dwarf")]
use crate::dwarf::DwarfResolver;
use crate::util::fstat;
use crate::ErrorExt as _;
use crate::Result;

use super::ElfParser;

// SAFETY: The provided value is non-zero.
const DFL_CACHE_MAX: NonZeroUsize = unsafe { NonZeroUsize::new_unchecked(1024) };

#[derive(Clone, Debug)]
pub(crate) enum ElfBackend {
    #[cfg(feature = "dwarf")]
    Dwarf(Rc<DwarfResolver>), // ELF w/ DWARF
    Elf(Rc<ElfParser>), // ELF w/o DWARF
}

#[cfg(test)]
#[cfg(feature = "dwarf")]
impl ElfBackend {
    pub fn to_dwarf(&self) -> Option<Rc<DwarfResolver>> {
        if let Self::Dwarf(dwarf) = self {
            Some(Rc::clone(dwarf))
        } else {
            None
        }
    }

    pub fn is_dwarf(&self) -> bool {
        matches!(self, Self::Dwarf(_))
    }
}

#[derive(Debug)]
struct ElfCacheEntry {
    dev: libc::dev_t,
    inode: libc::ino_t,
    size: libc::off_t,
    mtime_sec: libc::time_t,
    mtime_nsec: i64,
    backend: ElfBackend,
}

impl ElfCacheEntry {
    pub fn new(
        file: File,
        line_number_info: bool,
        debug_info_symbols: bool,
    ) -> Result<ElfCacheEntry> {
        let stat = fstat(file.as_raw_fd())?;
        let parser = Rc::new(ElfParser::open_file(file)?);

        #[cfg(feature = "dwarf")]
        let backend = ElfBackend::Dwarf(Rc::new(DwarfResolver::from_parser(
            Rc::clone(&parser),
            line_number_info,
            debug_info_symbols,
        )?));

        #[cfg(not(feature = "dwarf"))]
        let backend = ElfBackend::Elf(parser);

        Ok(ElfCacheEntry {
            dev: stat.st_dev,
            inode: stat.st_ino,
            size: stat.st_size,
            mtime_sec: stat.st_mtime,
            mtime_nsec: stat.st_mtime_nsec,
            backend,
        })
    }

    fn is_valid(&self, stat: &libc::stat) -> bool {
        stat.st_dev == self.dev
            && stat.st_ino == self.inode
            && stat.st_size == self.size
            && stat.st_mtime == self.mtime_sec
            && stat.st_mtime_nsec == self.mtime_nsec
    }

    fn get_backend(&self) -> ElfBackend {
        self.backend.clone()
    }
}


#[derive(Debug)]
struct _ElfCache {
    #[cfg(feature = "lru")]
    cache: LruCache<PathBuf, ElfCacheEntry>,
    line_number_info: bool,
    debug_info_symbols: bool,
}

impl _ElfCache {
    fn new(line_number_info: bool, debug_info_symbols: bool) -> _ElfCache {
        _ElfCache {
            #[cfg(feature = "lru")]
            cache: LruCache::new(DFL_CACHE_MAX),
            line_number_info,
            debug_info_symbols,
        }
    }

    #[cfg(feature = "lru")]
    fn find_or_create_backend(&mut self, file_name: &Path, file: File) -> Result<ElfBackend> {
        if let Some(entry) = self.cache.get(file_name) {
            let stat = fstat(file.as_raw_fd())?;

            if entry.is_valid(&stat) {
                return Ok(entry.get_backend())
            }
        }

        let entry = ElfCacheEntry::new(file, self.line_number_info, self.debug_info_symbols)?;
        let backend = entry.get_backend();
        let _previous = self.cache.put(file_name.to_path_buf(), entry);
        Ok(backend)
    }

    #[cfg(not(feature = "lru"))]
    fn find_or_create_backend(&mut self, _file_name: &Path, file: File) -> Result<ElfBackend> {
        let entry = ElfCacheEntry::new(file, self.line_number_info, self.debug_info_symbols)?;
        let backend = entry.get_backend();
        Ok(backend)
    }

    pub fn find(&mut self, path: &Path) -> Result<ElfBackend> {
        let file = File::open(path)
            .with_context(|| format!("failed to open ELF file {}", path.display()))?;
        self.find_or_create_backend(path, file)
    }
}

#[derive(Debug)]
pub(crate) struct ElfCache {
    cache: RefCell<_ElfCache>,
}

impl ElfCache {
    pub fn new(line_number_info: bool, debug_info_symbols: bool) -> ElfCache {
        ElfCache {
            cache: RefCell::new(_ElfCache::new(line_number_info, debug_info_symbols)),
        }
    }

    pub fn find(&self, path: &Path) -> Result<ElfBackend> {
        let mut cache = self.cache.borrow_mut();
        cache.find(path)
    }
}

#[cfg(test)]
#[cfg(feature = "dwarf")]
mod tests {
    use super::*;

    use std::env;
    use std::ptr;

    use test_log::test;

    #[test]
    fn test_cache() {
        let bin_name = Path::new(&env!("CARGO_MANIFEST_DIR"))
            .join("data")
            .join("test-no-debug.bin");

        let code_info = true;
        let debug_syms = false;
        let cache = ElfCache::new(code_info, debug_syms);
        let backend_first = cache.find(Path::new(&bin_name));
        let backend_second = cache.find(Path::new(&bin_name));
        assert!(backend_first.is_ok());
        assert!(backend_second.is_ok());
        let backend_first = backend_first.unwrap();
        let backend_second = backend_second.unwrap();
        assert!(backend_first.is_dwarf());
        assert!(backend_second.is_dwarf());
        assert_eq!(
            ptr::addr_of!(*backend_first.to_dwarf().unwrap().get_parser()),
            ptr::addr_of!(*backend_second.to_dwarf().unwrap().get_parser())
        );
    }
}
