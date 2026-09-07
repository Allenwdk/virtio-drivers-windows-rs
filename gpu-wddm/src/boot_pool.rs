//! DroidVM GPU feature 7: fixed boot-shared DRM pool, device config bytes 16..48.
//! No physical address is guessed from RAM size or from a map response.

pub const MAP_INFO_POOL: u32 = 1 << 31;

// For pool blobs the host owns placement and returns its byte offset in the
// response's padding word. The requested BAR offset only applies to BAR maps.
pub fn mapping_offset(map_info: u32, pool_offset: u32, bar_offset: u64) -> u64 {
    if map_info & MAP_INFO_POOL != 0 {
        u64::from(pool_offset)
    } else {
        bar_offset
    }
}

// VirtIO PCI only guarantees 32-bit config accesses. In particular, passing
// u64 to PciTransport::read_config_space panics on ARM64 before any MMIO read.
pub fn read_config_u64<E>(
    offset: usize,
    mut read_u32: impl FnMut(usize) -> Result<u32, E>,
) -> Result<u64, E> {
    let low = u32::from_le(read_u32(offset)?);
    let high = u32::from_le(read_u32(offset + 4)?);
    Ok(u64::from(low) | (u64::from(high) << 32))
}

#[derive(Clone, Copy, Debug)]
pub struct BootPool {
    pub base: u64,
    pub size: u64,
}

impl BootPool {
    pub fn new(base: u64, size: u64) -> Option<Self> {
        if base == 0 || size == 0 || (base | size) & 4095 != 0
            || base.checked_add(size).is_none() || size > (1u64 << 32)
        {
            return None;
        }
        Some(Self { base, size })
    }

    pub fn physical_range(&self, offset: u64, size: u64) -> Option<u64> {
        if size == 0 || (offset | size) & 4095 != 0
            || offset.checked_add(size)? > self.size
        {
            return None;
        }
        self.base.checked_add(offset)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pool_mapping_uses_host_placement_instead_of_bar_slot() {
        assert_eq!(mapping_offset(MAP_INFO_POOL | 1, 0x200000, 0), 0x200000);
        assert_eq!(mapping_offset(MAP_INFO_POOL | 1, 0x400000, 0x4000), 0x400000);
        assert_eq!(mapping_offset(1, 0, 0x4000), 0x4000);
        assert_eq!(mapping_offset(3, 0x200000, 0x8000), 0x8000);
    }
    #[test]
    fn config_reads_are_dwords_and_preserve_high_address_bits() {
        let mut offsets = Vec::new();
        let value = read_config_u64(32, |offset| {
            offsets.push(offset);
            match offset {
                32 => Ok(0x7c000000u32.to_le()),
                36 => Ok(1u32.to_le()),
                _ => Err(()),
            }
        }).unwrap();
        assert_eq!(value, 0x17c000000);
        assert_eq!(offsets, [32, 36]);
        let error = read_config_u64(40, |offset| {
            if offset == 40 { Ok(0u32) } else { Err("config too short") }
        });
        assert_eq!(error, Err("config too short"));
    }
    #[test]
    fn rejects_wraparound_unaligned_and_out_of_pool() {
        let p = BootPool::new(0x17c000000, 0x4000000).unwrap();
        assert_eq!(p.physical_range(0x200000, 0x4000), Some(0x17c200000));
        assert_eq!(p.physical_range(0x3fff000, 0x1000), Some(0x17ffff000));
        for (offset, size) in [(0x4000000, 4096), (0, 0), (1, 4096),
                               (0, 1), (u64::MAX - 4095, 8192)] {
            assert_eq!(p.physical_range(offset, size), None);
        }
        assert!(BootPool::new(u64::MAX - 4095, 8192).is_none());
        assert!(BootPool::new(0, 4096).is_none());
    }
}
