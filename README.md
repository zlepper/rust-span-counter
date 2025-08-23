# rust-span-counter

Parses Rust source files to extract word-by-word character spans from string literals.

## Usage

```bash
rust-span-counter <FILE> <LINE_NUM>
```

Given `example.rs`:
```rust
fn main() {
    let s = "hello world test";
}
```

```bash
$ rust-span-counter example.rs 2
"hello" | 0-5
"world" | 6-11
"test" | 12-16
```

## Installation

```bash
# Via cargo
cargo install --path .

# Or run directly
cargo run -- <file> <line_number>
```

## Output Format

`"word" | start_pos-end_pos` where positions are 0-based character indices within the string (end position is exclusive).

## Error Handling

- `No string found on the specified line` - Line contains no string literals
- `Multiple strings found on the same line` - Line contains more than one string literal
- `File error` or `Parse error` - File doesn't exist or contains invalid Rust syntax