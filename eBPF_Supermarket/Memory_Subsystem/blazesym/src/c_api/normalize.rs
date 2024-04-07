use std::ffi::CString;
use std::ffi::OsString;
use std::fmt::Debug;
use std::fmt::Formatter;
use std::fmt::Result as FmtResult;
use std::mem::ManuallyDrop;
use std::os::raw::c_char;
use std::os::unix::ffi::OsStringExt as _;
use std::path::PathBuf;
use std::ptr;
use std::slice;

use crate::log::error;
use crate::normalize::ApkElf;
use crate::normalize::Elf;
use crate::normalize::NormalizedUserAddrs;
use crate::normalize::Normalizer;
use crate::normalize::Unknown;
use crate::normalize::UserAddrMeta;
use crate::util::slice_from_user_array;
use crate::Addr;


/// Create an instance of a blazesym normalizer.
///
/// The returned pointer should be released using
/// [`blaze_normalizer_free`] once it is no longer needed.
#[no_mangle]
pub extern "C" fn blaze_normalizer_new() -> *mut Normalizer {
    let normalizer = Normalizer::new();
    let normalizer_box = Box::new(normalizer);
    Box::into_raw(normalizer_box)
}


/// Free a blazesym normalizer.
///
/// Release resources associated with a normalizer as created by
/// [`blaze_normalizer_new`], for example.
///
/// # Safety
/// The provided normalizer should have been created by
/// [`blaze_normalizer_new`].
#[no_mangle]
pub unsafe extern "C" fn blaze_normalizer_free(normalizer: *mut Normalizer) {
    if !normalizer.is_null() {
        // SAFETY: The caller needs to ensure that `normalizer` is a
        //         valid pointer.
        drop(unsafe { Box::from_raw(normalizer) });
    }
}


/// A normalized address along with an index into the associated
/// [`blaze_user_addr_meta`] array (such as
/// [`blaze_normalized_user_addrs::metas`]).
#[repr(C)]
#[derive(Debug)]
pub struct blaze_normalized_addr {
    /// The normalized address.
    pub addr: Addr,
    /// The index into the associated [`blaze_user_addr_meta`] array.
    pub meta_idx: usize,
}

impl From<(Addr, usize)> for blaze_normalized_addr {
    fn from((addr, meta_idx): (Addr, usize)) -> Self {
        Self { addr, meta_idx }
    }
}


/// The valid variant kind in [`blaze_user_addr_meta`].
#[repr(C)]
#[derive(Debug)]
pub enum blaze_user_addr_meta_kind {
    /// [`blaze_user_addr_meta_variant::unknown`] is valid.
    BLAZE_USER_ADDR_UNKNOWN,
    /// [`blaze_user_addr_meta_variant::apk_elf`] is valid.
    BLAZE_USER_ADDR_APK_ELF,
    /// [`blaze_user_addr_meta_variant::elf`] is valid.
    BLAZE_USER_ADDR_ELF,
}


/// C compatible version of [`ApkElf`].
#[repr(C)]
#[derive(Debug)]
pub struct blaze_user_addr_meta_apk_elf {
    /// The canonical absolute path to the APK, including its name.
    /// This member is always present.
    pub apk_path: *mut c_char,
    /// The relative path to the ELF file inside the APK.
    pub elf_path: *mut c_char,
    /// The length of the build ID, in bytes.
    pub elf_build_id_len: usize,
    /// The optional build ID of the ELF file, if found.
    pub elf_build_id: *mut u8,
}

impl From<ApkElf> for blaze_user_addr_meta_apk_elf {
    fn from(other: ApkElf) -> Self {
        let ApkElf {
            apk_path,
            elf_path,
            elf_build_id,
            _non_exhaustive: (),
        } = other;
        Self {
            apk_path: CString::new(apk_path.into_os_string().into_vec())
                .expect("encountered path with NUL bytes")
                .into_raw(),
            elf_path: CString::new(elf_path.into_os_string().into_vec())
                .expect("encountered path with NUL bytes")
                .into_raw(),
            elf_build_id_len: elf_build_id
                .as_ref()
                .map(|build_id| build_id.len())
                .unwrap_or(0),
            elf_build_id: elf_build_id
                .map(|build_id| {
                    // SAFETY: We know the pointer is valid because it
                    //         came from a `Box`.
                    unsafe {
                        Box::into_raw(build_id.into_boxed_slice())
                            .as_mut()
                            .unwrap()
                            .as_mut_ptr()
                    }
                })
                .unwrap_or_else(ptr::null_mut),
        }
    }
}

impl From<blaze_user_addr_meta_apk_elf> for ApkElf {
    fn from(other: blaze_user_addr_meta_apk_elf) -> Self {
        let blaze_user_addr_meta_apk_elf {
            apk_path,
            elf_path,
            elf_build_id_len,
            elf_build_id,
        } = other;

        ApkElf {
            apk_path: PathBuf::from(OsString::from_vec(
                unsafe { CString::from_raw(apk_path) }.into_bytes(),
            )),
            elf_path: PathBuf::from(OsString::from_vec(
                unsafe { CString::from_raw(elf_path) }.into_bytes(),
            )),
            elf_build_id: (!elf_build_id.is_null()).then(|| unsafe {
                Box::<[u8]>::from_raw(slice::from_raw_parts_mut(elf_build_id, elf_build_id_len))
                    .into_vec()
            }),
            _non_exhaustive: (),
        }
    }
}


/// C compatible version of [`Elf`].
#[repr(C)]
#[derive(Debug)]
pub struct blaze_user_addr_meta_elf {
    /// The path to the ELF file. This member is always present.
    pub path: *mut c_char,
    /// The length of the build ID, in bytes.
    pub build_id_len: usize,
    /// The optional build ID of the ELF file, if found.
    pub build_id: *mut u8,
}

impl From<Elf> for blaze_user_addr_meta_elf {
    fn from(other: Elf) -> Self {
        let Elf {
            path,
            build_id,
            _non_exhaustive: (),
        } = other;
        Self {
            path: CString::new(path.into_os_string().into_vec())
                .expect("encountered path with NUL bytes")
                .into_raw(),
            build_id_len: build_id
                .as_ref()
                .map(|build_id| build_id.len())
                .unwrap_or(0),
            build_id: build_id
                .map(|build_id| {
                    // SAFETY: We know the pointer is valid because it
                    //         came from a `Box`.
                    unsafe {
                        Box::into_raw(build_id.into_boxed_slice())
                            .as_mut()
                            .unwrap()
                            .as_mut_ptr()
                    }
                })
                .unwrap_or_else(ptr::null_mut),
        }
    }
}

impl From<blaze_user_addr_meta_elf> for Elf {
    fn from(other: blaze_user_addr_meta_elf) -> Self {
        let blaze_user_addr_meta_elf {
            path,
            build_id_len,
            build_id,
        } = other;

        Elf {
            path: PathBuf::from(OsString::from_vec(
                unsafe { CString::from_raw(path) }.into_bytes(),
            )),
            build_id: (!build_id.is_null()).then(|| unsafe {
                Box::<[u8]>::from_raw(slice::from_raw_parts_mut(build_id, build_id_len)).into_vec()
            }),
            _non_exhaustive: (),
        }
    }
}


/// C compatible version of [`Unknown`].
#[repr(C)]
#[derive(Debug)]
pub struct blaze_user_addr_meta_unknown {
    /// This member is unused.
    pub _unused: u8,
}

impl From<Unknown> for blaze_user_addr_meta_unknown {
    fn from(other: Unknown) -> Self {
        let Unknown {
            _non_exhaustive: (),
        } = other;
        Self { _unused: 0 }
    }
}

impl From<blaze_user_addr_meta_unknown> for Unknown {
    fn from(other: blaze_user_addr_meta_unknown) -> Self {
        let blaze_user_addr_meta_unknown { _unused } = other;
        Unknown {
            _non_exhaustive: (),
        }
    }
}


/// The actual variant data in [`blaze_user_addr_meta`].
#[repr(C)]
pub union blaze_user_addr_meta_variant {
    /// Valid on [`blaze_user_addr_meta_kind::BLAZE_USER_ADDR_APK_ELF`].
    pub apk_elf: ManuallyDrop<blaze_user_addr_meta_apk_elf>,
    /// Valid on [`blaze_user_addr_meta_kind::BLAZE_USER_ADDR_ELF`].
    pub elf: ManuallyDrop<blaze_user_addr_meta_elf>,
    /// Valid on [`blaze_user_addr_meta_kind::BLAZE_USER_ADDR_UNKNOWN`].
    pub unknown: ManuallyDrop<blaze_user_addr_meta_unknown>,
}

impl Debug for blaze_user_addr_meta_variant {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.debug_struct(stringify!(blaze_user_addr_meta_variant))
            .finish()
    }
}


/// C ABI compatible version of [`UserAddrMeta`].
#[repr(C)]
#[derive(Debug)]
pub struct blaze_user_addr_meta {
    /// The variant kind that is present.
    pub kind: blaze_user_addr_meta_kind,
    /// The actual variant with its data.
    pub variant: blaze_user_addr_meta_variant,
}

impl From<UserAddrMeta> for blaze_user_addr_meta {
    fn from(other: UserAddrMeta) -> Self {
        match other {
            UserAddrMeta::ApkElf(apk_elf) => Self {
                kind: blaze_user_addr_meta_kind::BLAZE_USER_ADDR_APK_ELF,
                variant: blaze_user_addr_meta_variant {
                    apk_elf: ManuallyDrop::new(blaze_user_addr_meta_apk_elf::from(apk_elf)),
                },
            },
            UserAddrMeta::Elf(elf) => Self {
                kind: blaze_user_addr_meta_kind::BLAZE_USER_ADDR_ELF,
                variant: blaze_user_addr_meta_variant {
                    elf: ManuallyDrop::new(blaze_user_addr_meta_elf::from(elf)),
                },
            },
            UserAddrMeta::Unknown(unknown) => Self {
                kind: blaze_user_addr_meta_kind::BLAZE_USER_ADDR_UNKNOWN,
                variant: blaze_user_addr_meta_variant {
                    unknown: ManuallyDrop::new(blaze_user_addr_meta_unknown::from(unknown)),
                },
            },
        }
    }
}

impl From<blaze_user_addr_meta> for UserAddrMeta {
    fn from(other: blaze_user_addr_meta) -> Self {
        match other.kind {
            blaze_user_addr_meta_kind::BLAZE_USER_ADDR_APK_ELF => {
                UserAddrMeta::ApkElf(ApkElf::from(ManuallyDrop::into_inner(unsafe {
                    other.variant.apk_elf
                })))
            }
            blaze_user_addr_meta_kind::BLAZE_USER_ADDR_ELF => {
                UserAddrMeta::Elf(Elf::from(ManuallyDrop::into_inner(unsafe {
                    other.variant.elf
                })))
            }
            blaze_user_addr_meta_kind::BLAZE_USER_ADDR_UNKNOWN => {
                UserAddrMeta::Unknown(Unknown::from(ManuallyDrop::into_inner(unsafe {
                    other.variant.unknown
                })))
            }
        }
    }
}


/// An object representing normalized user addresses.
///
/// C ABI compatible version of [`NormalizedUserAddrs`].
#[repr(C)]
#[derive(Debug)]
pub struct blaze_normalized_user_addrs {
    /// The number of [`blaze_user_addr_meta`] objects present in `metas`.
    pub meta_count: usize,
    /// An array of `meta_count` objects.
    pub metas: *mut blaze_user_addr_meta,
    /// The number of [`blaze_normalized_addr`] objects present in `addrs`.
    pub addr_count: usize,
    /// An array of `addr_count` objects.
    pub addrs: *mut blaze_normalized_addr,
}

impl From<NormalizedUserAddrs> for blaze_normalized_user_addrs {
    fn from(other: NormalizedUserAddrs) -> Self {
        Self {
            meta_count: other.meta.len(),
            metas: unsafe {
                Box::into_raw(
                    other
                        .meta
                        .into_iter()
                        .map(blaze_user_addr_meta::from)
                        .collect::<Vec<_>>()
                        .into_boxed_slice(),
                )
                .as_mut()
                .unwrap()
                .as_mut_ptr()
            },
            addr_count: other.addrs.len(),
            addrs: unsafe {
                Box::into_raw(
                    other
                        .addrs
                        .into_iter()
                        .map(blaze_normalized_addr::from)
                        .collect::<Vec<_>>()
                        .into_boxed_slice(),
                )
                .as_mut()
                .unwrap()
                .as_mut_ptr()
            },
        }
    }
}


/// Normalize a list of user space addresses.
///
/// Contrary to [`blaze_normalize_user_addrs_sorted`] the provided
/// `addrs` array does not have to be sorted, but otherwise the
/// functions behave identically. If you happen to know that `addrs` is
/// sorted, using [`blaze_normalize_user_addrs_sorted`] instead will
/// result in slightly faster normalization.
///
/// C ABI compatible version of [`Normalizer::normalize_user_addrs`].
/// Returns `NULL` on error. The resulting object should be freed using
/// [`blaze_user_addrs_free`].
///
/// # Safety
/// Callers need to pass in a valid `addrs` pointer, pointing to memory of
/// `addr_count` addresses.
#[no_mangle]
pub unsafe extern "C" fn blaze_normalize_user_addrs(
    normalizer: *const Normalizer,
    addrs: *const Addr,
    addr_count: usize,
    pid: u32,
) -> *mut blaze_normalized_user_addrs {
    // SAFETY: The caller needs to ensure that `normalizer` is a valid
    //         pointer.
    let normalizer = unsafe { &*normalizer };
    // SAFETY: The caller needs to ensure that `addrs` is a valid pointer and
    //         that it points to `addr_count` elements.
    let addrs = unsafe { slice_from_user_array(addrs, addr_count) };
    let result = normalizer.normalize_user_addrs(addrs, pid.into());
    match result {
        Ok(addrs) => Box::into_raw(Box::new(blaze_normalized_user_addrs::from(addrs))),
        Err(err) => {
            error!("failed to normalize user addresses: {err}");
            ptr::null_mut()
        }
    }
}


/// Normalize a list of user space addresses.
///
/// The `addrs` array has to be sorted in ascending order. By providing
/// a pre-sorted array the library does not have to sort internally,
/// which will result in quicker normalization. If you don't have sorted
/// addresses, use [`blaze_normalize_user_addrs`] instead.
///
/// `pid` should describe the PID of the process to which the addresses
/// belongs. It may be `0` if they belong to the calling process.
///
/// C ABI compatible version of [`Normalizer::normalize_user_addrs_sorted`].
/// Returns `NULL` on error. The resulting object should be freed using
/// [`blaze_user_addrs_free`].
///
/// # Safety
/// Callers need to pass in a valid `addrs` pointer, pointing to memory of
/// `addr_count` addresses.
#[no_mangle]
pub unsafe extern "C" fn blaze_normalize_user_addrs_sorted(
    normalizer: *const Normalizer,
    addrs: *const Addr,
    addr_count: usize,
    pid: u32,
) -> *mut blaze_normalized_user_addrs {
    // SAFETY: The caller needs to ensure that `normalizer` is a valid
    //         pointer.
    let normalizer = unsafe { &*normalizer };
    // SAFETY: The caller needs to ensure that `addrs` is a valid pointer and
    //         that it points to `addr_count` elements.
    let addrs = unsafe { slice_from_user_array(addrs, addr_count) };
    let result = normalizer.normalize_user_addrs_sorted(addrs, pid.into());
    match result {
        Ok(addrs) => Box::into_raw(Box::new(blaze_normalized_user_addrs::from(addrs))),
        Err(err) => {
            error!("failed to normalize user addresses: {err}");
            ptr::null_mut()
        }
    }
}

/// Free an object as returned by [`blaze_normalized_user_addrs`] or
/// [`blaze_normalize_user_addrs_sorted`].
///
/// # Safety
/// The provided object should have been created by
/// [`blaze_normalized_user_addrs`] or
/// [`blaze_normalize_user_addrs_sorted`].
#[no_mangle]
pub unsafe extern "C" fn blaze_user_addrs_free(addrs: *mut blaze_normalized_user_addrs) {
    if addrs.is_null() {
        return
    }

    // SAFETY: The caller should make sure that `addrs` was created by
    //         `blaze_normalize_user_addrs_sorted`.
    let user_addrs = unsafe { Box::from_raw(addrs) };
    let addr_metas = unsafe {
        Box::<[blaze_user_addr_meta]>::from_raw(slice::from_raw_parts_mut(
            user_addrs.metas,
            user_addrs.meta_count,
        ))
    }
    .into_vec();
    let _norm_addrs = unsafe {
        Box::<[blaze_normalized_addr]>::from_raw(slice::from_raw_parts_mut(
            user_addrs.addrs,
            user_addrs.addr_count,
        ))
    }
    .into_vec();

    for addr_meta in addr_metas {
        let _meta = UserAddrMeta::from(addr_meta);
    }
}


#[cfg(test)]
mod tests {
    use super::*;


    /// Exercise the `Debug` representation of various types.
    #[test]
    fn debug_repr() {
        let norm_addr = blaze_normalized_addr {
            addr: 0x1337,
            meta_idx: 1,
        };
        assert_eq!(
            format!("{norm_addr:?}"),
            "blaze_normalized_addr { addr: 4919, meta_idx: 1 }"
        );

        let meta_kind = blaze_user_addr_meta_kind::BLAZE_USER_ADDR_APK_ELF;
        assert_eq!(format!("{meta_kind:?}"), "BLAZE_USER_ADDR_APK_ELF");

        let apk_elf = blaze_user_addr_meta_apk_elf {
            apk_path: ptr::null_mut(),
            elf_path: ptr::null_mut(),
            elf_build_id_len: 0,
            elf_build_id: ptr::null_mut(),
        };
        assert_eq!(
            format!("{apk_elf:?}"),
            "blaze_user_addr_meta_apk_elf { apk_path: 0x0, elf_path: 0x0, elf_build_id_len: 0, elf_build_id: 0x0 }",
        );

        let elf = blaze_user_addr_meta_elf {
            path: ptr::null_mut(),
            build_id_len: 0,
            build_id: ptr::null_mut(),
        };
        assert_eq!(
            format!("{elf:?}"),
            "blaze_user_addr_meta_elf { path: 0x0, build_id_len: 0, build_id: 0x0 }",
        );

        let unknown = blaze_user_addr_meta_unknown { _unused: 42 };
        assert_eq!(
            format!("{unknown:?}"),
            "blaze_user_addr_meta_unknown { _unused: 42 }",
        );

        let meta = blaze_user_addr_meta {
            kind: blaze_user_addr_meta_kind::BLAZE_USER_ADDR_UNKNOWN,
            variant: blaze_user_addr_meta_variant {
                unknown: ManuallyDrop::new(blaze_user_addr_meta_unknown { _unused: 42 }),
            },
        };
        assert_eq!(
            format!("{meta:?}"),
            "blaze_user_addr_meta { kind: BLAZE_USER_ADDR_UNKNOWN, variant: blaze_user_addr_meta_variant }",
        );

        let user_addrs = blaze_normalized_user_addrs {
            meta_count: 0,
            metas: ptr::null_mut(),
            addr_count: 0,
            addrs: ptr::null_mut(),
        };
        assert_eq!(
            format!("{user_addrs:?}"),
            "blaze_normalized_user_addrs { meta_count: 0, metas: 0x0, addr_count: 0, addrs: 0x0 }",
        );
    }

    /// Check that we can convert an [`Unknown`] into a
    /// [`blaze_user_addr_meta_unknown`] and back.
    #[test]
    fn unknown_conversion() {
        let unknown = Unknown {
            _non_exhaustive: (),
        };

        let unknown_new = Unknown::from(blaze_user_addr_meta_unknown::from(unknown.clone()));
        assert_eq!(unknown_new, unknown);

        let meta = UserAddrMeta::Unknown(unknown_new);
        let meta_new = UserAddrMeta::from(blaze_user_addr_meta::from(meta.clone()));
        assert_eq!(meta_new, meta);
    }

    /// Check that we can convert an [`ApkElf`] into a
    /// [`blaze_user_addr_meta_apk_elf`] and back.
    #[test]
    fn apk_elf_conversion() {
        let apk = ApkElf {
            apk_path: PathBuf::from("/tmp/archive.apk"),
            elf_path: PathBuf::from("file.so"),
            elf_build_id: Some(vec![0x01, 0x02, 0x03, 0x04]),
            _non_exhaustive: (),
        };

        let apk_new = ApkElf::from(blaze_user_addr_meta_apk_elf::from(apk.clone()));
        assert_eq!(apk_new, apk);

        let apk = ApkElf {
            apk_path: PathBuf::new(),
            elf_path: PathBuf::new(),
            elf_build_id: None,
            _non_exhaustive: (),
        };

        let apk_new = ApkElf::from(blaze_user_addr_meta_apk_elf::from(apk.clone()));
        assert_eq!(apk_new, apk);

        let meta = UserAddrMeta::ApkElf(apk_new);
        let meta_new = UserAddrMeta::from(blaze_user_addr_meta::from(meta.clone()));
        assert_eq!(meta_new, meta);
    }

    /// Check that we can convert an [`Elf`] into a [`blaze_user_addr_meta_elf`]
    /// and back.
    #[test]
    fn elf_conversion() {
        let elf = Elf {
            path: PathBuf::from("/tmp/file.so"),
            build_id: Some(vec![0x01, 0x02, 0x03, 0x04]),
            _non_exhaustive: (),
        };

        let elf_new = Elf::from(blaze_user_addr_meta_elf::from(elf.clone()));
        assert_eq!(elf_new, elf);

        let meta = UserAddrMeta::Elf(elf_new);
        let meta_new = UserAddrMeta::from(blaze_user_addr_meta::from(meta.clone()));
        assert_eq!(meta_new, meta);
    }
}
