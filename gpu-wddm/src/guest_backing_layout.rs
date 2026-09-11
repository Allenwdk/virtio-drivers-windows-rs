//! Validate the physical layout before advertising 64 KiB DMA segments.
pub const CHUNK_SIZE: usize = 65536;

pub fn valid_chunks(pages: &[u64], bytes: usize, page_size: usize) -> bool {
    if page_size == 0 || CHUNK_SIZE % page_size != 0 || bytes == 0
        || bytes % CHUNK_SIZE != 0 || pages.len() != bytes / page_size {
        return false;
    }
    pages.chunks_exact(CHUNK_SIZE / page_size).all(|chunk| {
        chunk[0].checked_mul(page_size as u64)
            .is_some_and(|addr| addr % CHUNK_SIZE as u64 == 0)
            && chunk.iter().enumerate().all(|(i, &page)| chunk[0].checked_add(i as u64) == Some(page))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn separate_aligned_chunks_are_valid() {
        let pages: Vec<u64> = (16..32).chain(128..144).collect();
        assert!(valid_chunks(&pages, 2 * CHUNK_SIZE, 4096));
    }

    #[test]
    fn rejects_hole_within_chunk() {
        let mut pages: Vec<u64> = (16..32).collect();
        pages[8] = 100;
        assert!(!valid_chunks(&pages, CHUNK_SIZE, 4096));
    }

    #[test]
    fn rejects_misalignment_and_partial_allocation() {
        assert!(!valid_chunks(&(17..33).collect::<Vec<_>>(), CHUNK_SIZE, 4096));
        assert!(!valid_chunks(&(16..31).collect::<Vec<_>>(), CHUNK_SIZE, 4096));
        assert!(!valid_chunks(&[], 0, 4096));
    }

    #[test]
    fn validates_other_page_sizes_and_overflow() {
        assert!(valid_chunks(&[4, 5, 6, 7, 16, 17, 18, 19], 2 * CHUNK_SIZE, 16384));
        assert!(valid_chunks(&[1, 5], 2 * CHUNK_SIZE, 65536));
        assert!(!valid_chunks(&[u64::MAX], CHUNK_SIZE, 65536));
        assert!(!valid_chunks(&[], CHUNK_SIZE, 0));
    }
}
