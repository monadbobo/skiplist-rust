mod arena;

use std::fmt::Debug;
use std::iter::Iterator;
use std::ptr;
use std::ptr::{null_mut, NonNull};
use std::sync::atomic::{AtomicPtr, Ordering};
use rand::{Rng, SeedableRng};
use rand::rngs::StdRng;
use crate::arena::Arena;

const MAX_HEIGHT: usize = 12;
const K_BRANCHING: usize = 4;

pub struct Node<K> {
    key: K,
    next: Vec<AtomicPtr<Node<K>>>,
}

impl<K> Node<K> {
    fn new(key: K, height: usize) -> Self {
        let mut next = Vec::with_capacity(height);
        for _ in 0..height {
            next.push(AtomicPtr::new(ptr::null_mut()));
        }
        Node { key, next }
    }

    fn next(&self, level: usize) -> *mut Node<K> {
        self.next[level].load(Ordering::Acquire)
    }

    fn set_next(&self, level: usize, node: *mut Node<K>) {
        self.next[level].store(node, Ordering::Release);
    }

    fn no_barrier_next(&self, level: usize) -> *mut Node<K> {
        self.next[level].load(Ordering::Relaxed)
    }

    fn no_barrier_set_next(&self, level: usize, node: *mut Node<K>) {
        self.next[level].store(node, Ordering::Relaxed);
    }
}


pub struct SkipListIterator<'a, K: Ord + Debug + Default> {
    node: *mut Node<K>,
    list: &'a SkipList<K>,
}

impl<'a, K: Ord + Debug + Default> SkipListIterator<'a, K> {
    pub fn new(list: &'a SkipList<K>) -> Self {
        SkipListIterator { node: null_mut(), list }
    }

    pub fn valid(&self) -> bool {
        !self.node.is_null()
    }

    pub fn key(&self) -> &K {
        assert!(self.valid());
        unsafe { &self.node.as_ref().unwrap().key }
    }

    pub fn next(&mut self) {
        assert!(self.valid());
        self.node = unsafe { self.node.as_ref().unwrap().next(0) };
    }

    pub fn prev(&mut self) {
        assert!(self.valid());
        self.node = self.list.find_less_than(self.key()).as_ptr();
        if self.node == self.list.head.as_ptr() {
            self.node = null_mut();
        }
    }

    pub fn seek(&mut self, target: &K) {
        self.node = self.list.find_greater_or_equal(target, &mut None);
    }

    pub fn seek_to_first(&mut self) {
        self.node = unsafe { self.list.head.as_ref().next(0) };
    }

    pub fn seek_to_last(&mut self) {
        self.node = self.list.find_last().as_ptr();
        if self.node == self.list.head.as_ptr() {
            self.node = null_mut();
        }
    }
}

pub struct SkipList<K: Ord + Debug + Default> {
    head: NonNull<Node<K>>,
    max_height: std::sync::atomic::AtomicUsize,
    rnd: StdRng,
    arena: Arena,
}

unsafe impl<K: Ord + Debug + Default + Send> Send for SkipList<K> {}
unsafe impl<K: Ord + Debug + Default + Sync> Sync for SkipList<K> {}

// impl<K: Ord + Debug + Default> Default for SkipList<K> {
//     fn default() -> Self {
//         Self::new()
//     }
// }

impl<K: Ord + Debug + Default> SkipList<K> {
    pub fn new(mut arena: Arena) -> SkipList<K> {
        let head = unsafe {
            let layout = std::alloc::Layout::new::<Node<K>>();
            let ptr = arena.allocate(layout.size()) as *mut Node<K>;
            ptr::write(ptr, Node::new(K::default(), MAX_HEIGHT));
            NonNull::new_unchecked(ptr)
        };
        let mut s = SkipList {
            head,
            max_height: std::sync::atomic::AtomicUsize::new(1),
            rnd: StdRng::seed_from_u64(0xdeadbeef),
            arena,
        };

        for i in 0..MAX_HEIGHT {
            unsafe {
                s.head.as_mut().set_next(i, ptr::null_mut());
            }
        }
        s
    }

    /// # Safety
    ///
    /// This function should not be called before data ready.
    pub unsafe fn key_is_after_node(&self, key: &K, node: *const Node<K>) -> bool {
        unsafe {
            node.as_ref().map(|n| &n.key)
                .map_or(false, |node_key| node_key < key)
        }
    }

    pub fn find_greater_or_equal(&self, key: &K, prev: &mut Option<&mut Vec<*mut Node<K>>>) -> *mut Node<K> {
        let mut x = self.head.as_ptr();
        let mut level = self.get_max_height() - 1;
        loop {
            let next = unsafe { x.as_ref().unwrap().next(level) };
            if unsafe { self.key_is_after_node(key, next) } {
                x = next;
            } else {
                if let Some(prev_node) = prev {
                    prev_node[level] = x;
                }
                if level == 0 {
                    return next;
                } else {
                    level -= 1;
                }
            }
        }
    }

    pub fn find_less_than(&self, key: &K) -> NonNull<Node<K>> {
        let mut x = self.head;
        let mut level = self.get_max_height() - 1;
        loop {
            let next = unsafe { x.as_ref().next(level) };
            if next.is_null() || unsafe { next.as_ref().unwrap().key >= *key } {
                if level == 0 {
                    return x;
                } else {
                    level -= 1;
                }
            } else {
                x = unsafe { NonNull::new_unchecked(next) };
            }
        }
    }

    pub fn find_last(&self) -> NonNull<Node<K>> {
        let mut x = self.head;
        let mut level = self.get_max_height() - 1;
        loop {
            let next = unsafe { x.as_ref().next(level) };
            if next.is_null() {
                if level == 0 {
                    return x;
                } else {
                    level -= 1;
                }
            } else {
                x = unsafe { NonNull::new_unchecked(next) };
            }
        }
    }

    pub fn contains(&self, key: &K) -> bool {
        let x = self.find_greater_or_equal(key, &mut None);
        let x_ref = unsafe { x.as_ref() };
        match x_ref {
            None => false,
            Some(x_ref) => x_ref.key == *key,
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
        let mut prev = vec![ptr::null_mut(); MAX_HEIGHT];
        let x = self.find_greater_or_equal(&key, &mut Some(&mut prev));
        assert!(x.is_null() || unsafe { x.as_ref().unwrap().key != key });

        let height = self.random_height();
        if height > self.get_max_height() {
            let i = self.get_max_height();
            for p in prev.iter_mut().take(height).skip(i) {
                *p = self.head.as_ptr();
            }
            self.max_height.store(height, Ordering::Relaxed);
        }

        let new_node = unsafe {
            let layout = std::alloc::Layout::new::<Node<K>>();
            let ptr = self.arena.allocate(layout.size()) as *mut Node<K>;
            ptr::write(ptr, Node::new(key, height));
            &mut *ptr
        };
        //        let new_node = Box::new(Node::new(key, height));
        //        let new_node_ptr = Box::leak(new_node);
        for (i, p) in prev.iter().enumerate().take(height) {
            unsafe {
                new_node.no_barrier_set_next(i, p.as_ref().unwrap().no_barrier_next(i));
                p.as_ref().unwrap().set_next(i, new_node);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Condvar, Mutex};
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
    use std::thread;
    use rand::{random, Rng, SeedableRng};
    use crate::arena::Arena;
    use super::{SkipList, SkipListIterator};
    #[test]
    fn test_empty() {
        let arena = Arena::new();
        let list = super::SkipList::new(arena);
        assert_eq!(list.contains(&10), false);

        let mut iter = SkipListIterator::new(&list);
        assert_eq!(iter.valid(), false);
        iter.seek_to_first();
        assert_eq!(iter.valid(), false);
        iter.seek(&100);
        assert_eq!(iter.valid(), false);
        iter.seek_to_last();
        assert_eq!(iter.valid(), false);
    }

    #[test]
    fn insert_and_lookup() {
        let n = 2000;
        let r = 5000;
        let mut rnd = rand::thread_rng();
        let mut keys = std::collections::btree_set::BTreeSet::new();
        let arena = Arena::new();
        let mut list = super::SkipList::new(arena);

        for _ in 0..r {
            let key = rnd.gen_range(0..r);
            if keys.insert(key) {
                list.insert(key);
                continue;
            }
        }

        for i in 0..n {
            if list.contains(&i) {
                assert!(keys.contains(&i));
            } else {
                assert!(!keys.contains(&i));
            }
        }

        {
            let mut iter = SkipListIterator::new(&list);
            iter.seek_to_first();
            for i in 0..r {
                if keys.contains(&i) {
                    assert_eq!(iter.valid(), true);
                    assert_eq!(iter.key(), &i);
                    iter.next();
                }
            }
            assert_eq!(iter.valid(), false);
        }

        {
            let mut iter = SkipListIterator::new(&list);
            assert!(!iter.valid());

            iter.seek(&0);
            assert!(iter.valid());
            assert_eq!(keys.iter().next().unwrap(), iter.key());

            iter.seek_to_first();
            assert!(iter.valid());
            assert_eq!(keys.iter().next().unwrap(), iter.key());

            iter.seek_to_last();
            assert!(iter.valid());
            assert_eq!(keys.iter().rev().next().unwrap(), iter.key());
        }

        // Forward iteration test
        for i in 0..r {
            let mut iter = SkipListIterator::new(&list);
            iter.seek(&i);
            let mut model_iter = keys.range(i..);

            for _ in 0..3 {
                let v = model_iter.next();
                if v.is_none() {
                    assert!(!iter.valid());
                    break;
                } else {
                    assert!(iter.valid());
                    assert_eq!(v.unwrap(), iter.key());
                    iter.next();
                }
            }
        }

        // Backward iteration test
        {
            let mut iter = SkipListIterator::new(&list);
            iter.seek_to_last();

            for k in keys.iter().rev() {
                assert!(iter.valid());
                assert_eq!(k, iter.key());
                iter.prev();
            }

            assert!(!iter.valid());
        }
    }

    const K: u64 = 4;

    type Key = u64;

    fn key(key: Key) -> u64 { key >> 40 }
    fn gen(key: Key) -> u64 { (key >> 8) & 0xffffffff }
    fn hash(key: Key) -> u64 { key & 0xff }

    fn hash_numbers(k: u64, g: u64) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        k.hash(&mut hasher);
        g.hash(&mut hasher);
        hasher.finish()
    }

    fn make_key(k: u64, g: u64) -> Key {
        assert!(k <= K);
        assert!(g <= 0xffffffff);
        (k << 40) | (g << 8) | (hash_numbers(k, g) & 0xff)
    }

    fn is_valid_key(k: Key) -> bool {
        hash(k) == (hash_numbers(key(k), gen(k)) & 0xff)
    }

    fn random_target(rng: &mut impl Rng) -> Key {
        match rng.gen_range(0..10) {
            0 => make_key(0, 0),
            1 => make_key(K, 0),
            _ => make_key(rng.gen_range(0..K), 0),
        }
    }

    struct State {
        generation: Vec<AtomicU64>,
    }

    impl State {
        fn new() -> Self {
            let generation = (0..K).map(|_| AtomicU64::new(0)).collect();
            State { generation }
        }

        fn set(&self, k: usize, v: u64) {
            self.generation[k].store(v, Ordering::Release);
        }

        fn get(&self, k: usize) -> u64 {
            self.generation[k].load(Ordering::Acquire)
        }
    }

    struct ConcurrentTest {
        current: State,
        list: SkipList<Key>,
    }

    impl ConcurrentTest {
        fn new() -> Self {
            let arena = Arena::new();
            ConcurrentTest {
                current: State::new(),
                list: SkipList::new(arena),
            }
        }

        fn write_step(&mut self, rng: &mut impl Rng) {
            let k = rng.gen_range(0..K) as usize;
            let g = self.current.get(k) + 1;
            let key = make_key(k as u64, g);
            self.list.insert(key);
            self.current.set(k, g);
        }

        fn read_step(&self, rng: &mut impl Rng) {
            let initial_state = State::new();
            for k in 0..K as usize {
                initial_state.set(k, self.current.get(k));
            }

            let mut pos = random_target(rng);
            let mut iter = SkipListIterator::new(&self.list);
            iter.seek(&pos);

            loop {
                let current = if iter.valid() {
                    *iter.key()
                } else {
                    make_key(K, 0)
                };

                assert!(is_valid_key(current));
                assert!(pos <= current, "should not go backwards");

                while pos < current {
                    assert!(key(pos) < K);

                    if gen(pos) != 0 {
                        assert!(gen(pos) > initial_state.get(key(pos) as usize) as u64);
                    }

                    if key(pos) < key(current) {
                        pos = make_key(key(pos) + 1, 0);
                    } else {
                        pos = make_key(key(pos), gen(pos) + 1);
                    }
                }

                if !iter.valid() {
                    break;
                }

                if rng.gen_bool(0.5) {
                    iter.next();
                    pos = make_key(key(pos), gen(pos) + 1);
                } else {
                    let new_target = random_target(rng);
                    if new_target > pos {
                        pos = new_target;
                        iter.seek(&new_target);
                    }
                }
            }
        }
    }

    #[test]
    fn concurrent_without_threads() {
        let mut test = ConcurrentTest::new();
        let mut rng = rand::thread_rng();
        for _ in 0..10000 {
            test.read_step(&mut rng);
            test.write_step(&mut rng);
        }
    }

    struct TestState {
        t: Mutex<ConcurrentTest>,
        seed: u64,
        quit_flag: AtomicBool,
        state: Mutex<ReaderState>,
        state_cv: Condvar,
    }

    #[derive(PartialEq, Eq)]
    enum ReaderState {
        Starting,
        Running,
        Done,
    }

    impl TestState {
        fn new(seed: u64) -> Self {
            TestState {
                t: Mutex::new(ConcurrentTest::new()),
                seed,
                quit_flag: AtomicBool::new(false),
                state: Mutex::new(ReaderState::Starting),
                state_cv: Condvar::new(),
            }
        }

        fn wait(&self, s: ReaderState) {
            let mut state = self.state.lock().unwrap();
            while *state != s {
                state = self.state_cv.wait(state).unwrap();
            }
        }

        fn change(&self, s: ReaderState) {
            let mut state = self.state.lock().unwrap();
            *state = s;
            self.state_cv.notify_all();
        }
    }

    fn concurrent_reader(state: Arc<TestState>) {
        let mut rng = rand::rngs::StdRng::seed_from_u64(state.seed);
        state.change(ReaderState::Running);
        while !state.quit_flag.load(Ordering::Acquire) {
            state.t.lock().unwrap().read_step(&mut rng);
        }
        state.change(ReaderState::Done);
    }

    fn run_concurrent(run: u64) {
        let seed = random::<u64>() + (run * 100);
        let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
        let n = 1000;
        let k_size = 1000;

        for i in 0..n {
            if i % 100 == 0 {
                println!("Run {} of {}", i, n);
            }
            let state = Arc::new(TestState::new(seed + 1));
            let state_clone = state.clone();
            thread::spawn(move || concurrent_reader(state_clone));

            state.wait(ReaderState::Running);
            for _ in 0..k_size {
                state.t.lock().unwrap().write_step(&mut rng);
            }
            state.quit_flag.store(true, Ordering::Release);
            state.wait(ReaderState::Done);
        }
    }

    #[test]
    fn concurrent_1() { run_concurrent(1); }
    #[test]
    fn concurrent_2() { run_concurrent(2); }
    #[test]
    fn concurrent_3() { run_concurrent(3); }
    #[test]
    fn concurrent_4() { run_concurrent(4); }
    #[test]
    fn concurrent_5() { run_concurrent(5); }
}
