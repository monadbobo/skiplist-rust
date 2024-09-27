# Rust SkipList

A Rust implementation of the SkipList data structure, inspired by LevelDB's SkipList. This project provides a
concurrent, lock-free SkipList implementation that can be used for efficient key-value storage and retrieval.

## Features

- Lock-free concurrent operations
- Efficient insertion and lookup
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
skiplist-rust = "0.1.0"
```

Then you can use the SkipList in your Rust code:

```rust
use skiplist_rust::SkipList;

fn main() {
    let arena = Arena::new();
    let mut list = SkipList::new(arena);

    // Insert some values
    list.insert(5);
    list.insert(2);
    list.insert(8);

    // Check if a value exists
    assert!(list.contains(&5));
    assert!(!list.contains(&3));

    // Iterate over the list
    let mut iter = SkipListIterator::new(&list);
    while iter.valid() {
        println!("Key: {}", iter.key());
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
- `insert(key: K)`: Insert a key into the SkipList
- `contains(&key: &K) -> bool`: Check if a key exists in the SkipList
- `iter(&self) -> SkipListIterator<K>`: Get an iterator over the SkipList

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
for concurrent access and a similar probabilistic balancing strategy.

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## License

This project is licensed under the MIT License.

## Acknowledgments

- Inspired by LevelDB's SkipList implementation
- Built with Rust's powerful type system and memory safety guarantees
