use std::fmt::Debug;
use std::ptr;
use std::ptr::NonNull;
use std::sync::atomic::{AtomicPtr, Ordering};
use rand::{Rng, SeedableRng};
use rand::rngs::StdRng;

const MAX_HEIGHT: usize = 12;
const K_BRANCHING: usize = 4;

pub struct Node<K> {
    key: Option<K>,
    next: Vec<AtomicPtr<Node<K>>>,
}

impl<K> Node<K> {
    fn new(key: Option<K>,height: usize) -> Self {
        let mut next = Vec::with_capacity(height);
        for _ in 0..height {
            next.push(AtomicPtr::new(ptr::null_mut()));
        }
        Node { key, next }
    }

    fn next(&self, level: usize) -> *mut Node<K> {
        self.next[level].load(std::sync::atomic::Ordering::Acquire)
    }

    fn set_next(&self, level: usize, node: *mut Node<K>) {
        self.next[level].store(node, std::sync::atomic::Ordering::Release);
    }

    fn no_barrier_next(&self, level: usize) -> *mut Node<K> {
        self.next[level].load(std::sync::atomic::Ordering::Relaxed)
    }

    fn no_barrier_set_next(&self, level: usize, node: *mut Node<K>) {
        self.next[level].store(node, std::sync::atomic::Ordering::Relaxed);
    }
}


pub struct SkipList<K: Ord>{
    head: NonNull<Node<K>>,
    max_height: std::sync::atomic::AtomicUsize,
    rnd: StdRng,
}

impl<K: Ord+Debug> SkipList<K> {
    pub fn new() -> SkipList<K>{
        let head = Box::new(Node::new(None, MAX_HEIGHT));
        let head_ptr = NonNull::from(Box::leak(head));
        let s = SkipList {
            head: head_ptr,
            max_height: std::sync::atomic::AtomicUsize::new(1),
            rnd: StdRng::seed_from_u64(0xdeadbeef),
        };

        for i in 0..MAX_HEIGHT {
            unsafe {
                s.head.as_ref().set_next(i, ptr::null_mut());
            }
        }
        s
    }

    pub fn key_is_after_node(&self, key: &K, node: *const Node<K>) -> bool {
        if node.is_null() {
            false
        } else {
            unsafe {
                match (*node).key {
                    Some(ref node_key) => node_key < key,
                    None => false,
                }
            }
        }
    }

    pub fn find_greater_or_equal(&self, key: &K, prev: &mut Option<&mut Vec<NonNull<Node<K>>>>) -> NonNull<Node<K>> {
        let mut x = self.head.as_ptr();
        let mut level = self.max_height.load(Ordering::Acquire) - 1;
        loop {
            let x_ref = unsafe{x.as_ref().unwrap()};
            let next = x_ref.next(level);
            if self.key_is_after_node(key, next) {
                x = next;
            } else {
                if let Some(prev_node) = prev {
                    prev_node[level] = NonNull::new(x).unwrap();
                }
                if level == 0 {
                    return NonNull::new(x).unwrap();
                } else {
                    level -= 1;
                }
            }
        }
    }

    pub fn contains(&self, key: &K) -> bool {
        let x = self.find_greater_or_equal(key, &mut None);
        let x_ref = unsafe{x.as_ref()};
        if x_ref.key.as_ref() == Some(key) {
            true
        } else {
            false
        }

    }

    pub fn random_height(&mut self) -> usize {
        let mut height = 1;
        while height < MAX_HEIGHT && self.rnd.gen_range(0..K_BRANCHING) == 0 {
            height += 1;
        }
        height
    }

    #[inline]
    fn get_max_height(&self) -> usize {
        self.max_height.load(Ordering::Relaxed)
    }

    pub fn insert(&mut self, key: K) {
        let mut prev = vec![self.head; MAX_HEIGHT];
        let x = self.find_greater_or_equal(&key, &mut Some(&mut prev));
        let x_ref = unsafe{x.as_ref()};

        assert_eq!(x_ref.key.as_ref().unwrap(), &key);

        let height = self.random_height();
        if height > self.get_max_height() {
            self.max_height.store(height, Ordering::Relaxed);
        }

        let new_node = Box::new(Node::new(Some(key), height));
        let new_node_ptr = NonNull::from(Box::leak(new_node));
        for i in 0..height {
            unsafe {
                new_node_ptr.as_ref().no_barrier_set_next(i, prev[i].as_ref().no_barrier_next(i));
                prev[i].as_ref().set_next(i, new_node_ptr.as_ptr());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_empty() {
        let list = super::SkipList::new();
        assert_eq!(list.contains(&10), false);
    }
}
