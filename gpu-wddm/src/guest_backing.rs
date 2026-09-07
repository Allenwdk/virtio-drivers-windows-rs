//! Windows-owned contiguous backing for guest-allocated DRM BOs.
//! The current pseudo-unprotected VM shares ordinary guest RAM with the host.
//! Keep pages and the MDL alive until RESOURCE_UNREF and allocation teardown.
use core::ptr::{NonNull, null_mut};
use spin::mutex::SpinMutex;
use winresult::STATUS;
use crate::NtStatus;
use wdk::wdm::*;

pub struct GuestBacking {
    allocation: NonNull<u8>,
    pub physical: u64,
    pub size: u32,
    mdl: MdlOwned,
    mapping: SpinMutex<Option<(NonNull<u8>, usize)>>,
}
unsafe impl Send for GuestBacking {}
unsafe impl Sync for GuestBacking {}
impl core::fmt::Debug for GuestBacking {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("GuestBacking").field("physical", &self.physical)
            .field("size", &self.size).finish()
    }
}

impl GuestBacking {
    pub fn new(size: u64) -> Result<Self, NtStatus> {
        // A 64 KiB-aligned start works with 4/16/64 KiB host pages. The BO size
        // is negotiated separately; callers must provide a matching multiple.
        const ALIGN: u64 = 65536;
        if size == 0 || size > u32::MAX as u64 - ALIGN || size & (ALIGN - 1) != 0 {
            return Err(NtStatus(STATUS::INVALID_PARAMETER));
        }
        let raw = NonNull::new(unsafe {
            MmAllocateContiguousMemory((size + ALIGN) as _,
                LARGE_INTEGER { QuadPart: 0xFFFFFFFFFF }) as *mut u8
        }).ok_or(NtStatus(STATUS::NO_MEMORY))?;
        let physical = mm_get_physical_address(raw.as_ptr() as _);
        let offset = ((ALIGN - (physical & (ALIGN - 1))) & (ALIGN - 1)) as usize;
        let ptr = unsafe { raw.as_ptr().add(offset) };
        unsafe { core::ptr::write_bytes(ptr, 0, size as usize); }
        let mdl = unsafe { IoAllocateMdl(ptr as _, size as u32, 0, 0, null_mut()) };
        if mdl.is_null() {
            unsafe { MmFreeContiguousMemory(raw.as_ptr() as _); }
            return Err(NtStatus(STATUS::NO_MEMORY));
        }
        unsafe { MmBuildMdlForNonPagedPool(mdl); }
        Ok(Self { allocation: raw, physical: physical + offset as u64,
            size: size as u32, mdl: MdlOwned(mdl), mapping: SpinMutex::new(None) })
    }

    pub fn map(&self) -> Result<NonNull<u8>, NtStatus> {
        let mut mapping = self.mapping.lock();
        if mapping.is_some() { return Err(NtStatus(STATUS::ALREADY_COMMITTED)); }
        let ptr = match microseh::try_seh(|| mm_map_locked_pages_specify_cache(
            &self.mdl, true, MEMORY_CACHING_TYPE::MmCached, None)) {
            Ok(Some(ptr)) => ptr,
            Ok(None) => return Err(NtStatus(STATUS::NO_MEMORY)),
            Err(e) => return Err(e.into()),
        };
        *mapping = Some((ptr, unsafe { PsGetCurrentProcessId() as usize }));
        Ok(ptr)
    }

    pub fn unmap(&self, ptr: NonNull<u8>) -> Result<(), NtStatus> {
        let mut mapping = self.mapping.lock();
        if *mapping != Some((ptr, unsafe { PsGetCurrentProcessId() as usize })) {
            return Err(NtStatus(STATUS::INVALID_PARAMETER));
        }
        mm_unmap_locked_pages(&self.mdl, ptr);
        *mapping = None;
        Ok(())
    }
}

impl Drop for GuestBacking {
    fn drop(&mut self) {
        // A client is expected to unmap before close. If it does not, keep the
        // pages pinned rather than leave a live userspace alias to freed RAM.
        if self.mapping.get_mut().is_none() {
            unsafe { MmFreeContiguousMemory(self.allocation.as_ptr() as _); }
        }
    }
}
