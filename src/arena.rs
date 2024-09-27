use std::sync::atomic::{AtomicUsize, Ordering};
use std::ptr;

const BLOCK_SIZE: usize = 4096;

pub struct Arena {
    alloc_ptr: *mut u8,
    alloc_bytes_remaining: usize,
    blocks: Vec<*mut u8>,
    memory_usage: AtomicUsize,
}

impl Default for Arena {
    fn default() -> Self {
        Self::new()
    }
}

impl Arena {
    pub fn new() -> Self {
        Arena {
            alloc_ptr: ptr::null_mut(),
            alloc_bytes_remaining: 0,
            blocks: Vec::new(),
            memory_usage: AtomicUsize::new(0),
        }
    }

    pub fn allocate(&mut self, bytes: usize) -> *mut u8 {
        assert!(bytes > 0);
        if bytes <= self.alloc_bytes_remaining {
            unsafe {
                let result = self.alloc_ptr;
                self.alloc_ptr = self.alloc_ptr.add(bytes);
                self.alloc_bytes_remaining -= bytes;
                result
            }
        } else {
            self.allocate_fallback(bytes)
        }
    }

    pub fn allocate_aligned(&mut self, bytes: usize) -> *mut u8 {
        let align = if std::mem::size_of::<*mut ()>() > 8 {
            std::mem::size_of::<*mut ()>()
        } else {
            8
        };
        assert!(align.is_power_of_two());

        let current_mod = (self.alloc_ptr as usize) & (align - 1);
        let slop = if current_mod == 0 { 0 } else { align - current_mod };
        let needed = bytes + slop;

        if needed <= self.alloc_bytes_remaining {
            unsafe {
                let result = self.alloc_ptr.add(slop);
                self.alloc_ptr = self.alloc_ptr.add(needed);
                self.alloc_bytes_remaining -= needed;
                result
            }
        } else {
            self.allocate_fallback(bytes)
        }
    }

    pub fn memory_usage(&self) -> usize {
        self.memory_usage.load(Ordering::Relaxed)
    }

    fn allocate_fallback(&mut self, bytes: usize) -> *mut u8 {
        if bytes > BLOCK_SIZE / 4 {
            return self.allocate_new_block(bytes);
        }

        self.alloc_ptr = self.allocate_new_block(BLOCK_SIZE);
        self.alloc_bytes_remaining = BLOCK_SIZE;

        let result = self.alloc_ptr;
        unsafe {
            self.alloc_ptr = self.alloc_ptr.add(bytes);
        }
        self.alloc_bytes_remaining -= bytes;
        result
    }

    fn allocate_new_block(&mut self, block_bytes: usize) -> *mut u8 {
        let result = unsafe {
            let layout = std::alloc::Layout::from_size_align(block_bytes, 1).unwrap();
            std::alloc::alloc(layout)
        };
        self.blocks.push(result);
        self.memory_usage.fetch_add(block_bytes + std::mem::size_of::<*mut u8>(), Ordering::Relaxed);
        result
    }
}

impl Drop for Arena {
    fn drop(&mut self) {
        for &block in &self.blocks {
            unsafe {
                let layout = std::alloc::Layout::from_size_align(BLOCK_SIZE, 1).unwrap();
                std::alloc::dealloc(block, layout);
            }
        }
    }
}

mod test {
    use std::cmp;
    use rand::prelude::StdRng;
    use rand::{Rng, SeedableRng};
    use crate::arena::Arena;

    #[test]
    fn test_arena_empty() {
        let _arena = Arena::new();
    }

    #[test]
    fn test_arena_simple() {
        let mut allocated = Vec::new();
        let mut arena = Arena::new();
        let n = 100000;
        let mut bytes = 0;
        let mut rng = StdRng::seed_from_u64(301);

        for i in 0..n {
            let s = if i % (n / 10) == 0 {
                i
            } else if rng.gen_bool(1.0 / 4000.0) {
                rng.gen_range(0..6000)
            } else if rng.gen_bool(0.1) {
                rng.gen_range(0..100)
            } else {
                rng.gen_range(0..20)
            };

            let s = cmp::max(s, 1); // Our arena disallows size 0 allocations.

            let r = if rng.gen_bool(0.1) {
                arena.allocate_aligned(s)
            } else {
                arena.allocate(s)
            };

            unsafe {
                for b in 0..s {
                    // Fill the "i"th allocation with a known bit pattern
                    *r.add(b) = (i % 256) as u8;
                }
            }

            bytes += s;
            allocated.push((s, r));
            assert!(arena.memory_usage() >= bytes);
            if i > n / 10 {
                assert!(arena.memory_usage() <= (bytes as f64 * 1.10) as usize);
            }
        }

        for (i, &(num_bytes, p)) in allocated.iter().enumerate() {
            unsafe {
                for b in 0..num_bytes {
                    // Check the "i"th allocation for the known bit pattern
                    assert_eq!((*p.add(b) as i32) & 0xff, i as i32 % 256);
                }
            }
        }
    }
}