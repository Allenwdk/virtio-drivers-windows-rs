#[repr(C)]
#[derive(PartialEq, Eq, Clone, Copy, Debug)]
#[cfg_attr(not(test), derive(zerocopy::FromBytes, zerocopy::IntoBytes, zerocopy::Immutable))]
pub struct VirglResourceLayoutPlane {
    pub offset: u64,
    pub stride: u32,
    pub size: u32,
}

#[repr(C)]
#[derive(PartialEq, Eq, Clone, Copy, Debug)]
#[cfg_attr(not(test), derive(zerocopy::FromBytes, zerocopy::IntoBytes, zerocopy::Immutable))]
pub struct VirglResourceLayout {
    pub modifier: u64,
    pub num_planes: u32,
    pub reserved: u32,
    pub planes: [VirglResourceLayoutPlane; 4],
}


const _: () = assert!(core::mem::size_of::<VirglResourceLayout>() == 80);
impl VirglResourceLayout {
    pub fn is_valid(&self) -> bool {
        self.modifier != 0x00ff_ffff_ffff_ffff && self.reserved == 0 &&
            (1..=4).contains(&self.num_planes) &&
            self.planes[..self.num_planes as usize].iter().all(|p|
                p.stride != 0 && p.size != 0 && p.offset.checked_add(p.size as u64).is_some())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn valid() -> VirglResourceLayout {
        VirglResourceLayout { modifier: 0, num_planes: 1, reserved: 0,
            planes: [VirglResourceLayoutPlane { offset: 0, stride: 256, size: 16384 }; 4] }
    }
    #[test] fn real_linear_layout_is_accepted() { assert!(valid().is_valid()); }
    #[test] fn corrupt_plane_count_cannot_index_outside_response() {
        for n in [0, 5, u32::MAX] {
            let mut l = valid(); l.num_planes = n; assert!(!l.is_valid());
        }
    }
    #[test] fn unwritten_or_unknown_layout_is_rejected() {
        let mut l = valid(); l.modifier = 0x00ff_ffff_ffff_ffff; assert!(!l.is_valid());
        l = valid(); l.planes[0].stride = 0; assert!(!l.is_valid());
        l = valid(); l.planes[0].size = 0; assert!(!l.is_valid());
    }
    #[test] fn overflowing_plane_extent_is_rejected() {
        let mut l = valid(); l.planes[0].offset = u64::MAX; assert!(!l.is_valid());
    }
}
