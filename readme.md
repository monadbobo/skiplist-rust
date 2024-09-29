# Rust SkipList

A Rust implementation of the SkipList data structure, inspired by LevelDB's SkipList. This project provides a
SkipList implementation with lock-free reads and locked writes, suitable for efficient key-value storage and retrieval.

## Features

- Lock-free read operations
- Efficient insertion (with locking) and lookup
- Iterator support for traversal
- Configurable maximum height and branching factor
- Written in safe Rust with minimal unsafe code
- Memory management through a shared Arena allocator
- No explicit delete operation (following LevelDB's design)

## Memory Management

The SkipList uses a shared `Arena` for memory allocation. This means:

- All nodes are allocated from the Arena
- There's no need for manual memory deallocation
- The entire SkipList is deallocated when the Arena is dropped

## Usage

Add this to your `Cargo.toml`:

```toml
[dependencies]
skiplist-rust = "0.3.0"
```

Then you can use the SkipList in your Rust code:

```rust
use skiplist_rust::{SkipList, SkipListIterator};
use skiplist_rust::arena::Arena;
use std::sync::Arc;

fn main() {
    let arena = Arena::new();
    let skiplist = Arc::new(SkipList::new(arena));
    let mut write_handles = vec![];
    for i in 0..5 {
        let skiplist_clone = Arc::clone(&skiplist);
        let handle = thread::spawn(move || {
            let start = i * 100;
            let end = start + 100;
            for k in start..end {
                skiplist_clone.insert(k);
                println!("Thread {} inserted: {}", i, k);
            }
        });
        write_handles.push(handle);
    }

    let mut read_handles = vec![];
    for i in 0..3 {
        let skiplist_clone = Arc::clone(&skiplist);
        let handle = thread::spawn(move || {
            let mut rng = rand::thread_rng();
            let start = i * 100;
            let end = start + 100;
            for _ in start..end {
                let key = rng.gen_range(0..1000);
                let contains =  skiplist_clone.contains(&key);
                println!("Thread {} queried: {}, result: {}", i, key, contains);
                thread::sleep(Duration::from_millis(1));
            }
        });
        read_handles.push(handle);
    }

    for handle in write_handles {
        handle.join().unwrap();
    }

    for handle in read_handles {
        handle.join().unwrap();
    }

    let mut iter = skiplist.iter();
    iter.seek_to_first();
    println!("Final SkipList contents:");
    while iter.valid() {
        println!("{:?}", iter.key());
        iter.next();
    }    
}
```

## API

### `Arena`

- `new() -> Arena`: Create a new Arena
- `allocate(bytes: usize) -> *mut u8`: Allocate memory of the specified size
- `allocate_aligned(bytes: usize) -> *mut u8`: Allocate memory of the specified size with alignment
- `memory_usage(&self) -> usize`: Get the current memory usage of the arena

### `SkipList<K>`

- `new(arena: Arena) -> SkipList<K>`: Create a new SkipList

  Creates and returns a new `SkipList` instance using the provided memory arena.

- `insert(key: K)`: Insert a key into the SkipList (requires locking)

  Inserts the given key into the SkipList. This operation acquires a write lock to ensure thread-safe modification.

- `contains(&key: &K) -> bool`: Check if a key exists in the SkipList (lock-free)

  Checks whether the given key exists in the SkipList. This is a lock-free operation that allows concurrent reads.

- `iter(&self) -> SkipListIterator<K>`: Get an iterator over the SkipList (lock-free)

  Returns an iterator that can be used to traverse the elements in the SkipList. This operation is lock-free, allowing concurrent iteration with other operations.

### `SkipListIterator<K>`

- `new(list: &SkipList<K>) -> SkipListIterator<K>`: Create a new iterator over a SkipList
- `valid(&self) -> bool`: Check if the iterator is pointing to a valid node
- `key(&self) -> &K`: Get the key of the current node
- `next(&mut self)`: Move to the next node
- `prev(&mut self)`: Move to the previous node
- `seek(&mut self, target: &K)`: Seek to the first node with a key >= target
- `seek_to_first(&mut self)`: Seek to the first node
- `seek_to_last(&mut self)`: Seek to the last node

## Performance

This implementation aims to provide similar performance characteristics to LevelDB's SkipList. It uses atomic operations
for concurrent read access and locking for write operations, providing a balance between concurrency and data
consistency.

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## License

This project is licensed under the MIT License.

## Acknowledgments

- Inspired by LevelDB's SkipList implementation
- Built with Rust's powerful type system and memory safety guarantees