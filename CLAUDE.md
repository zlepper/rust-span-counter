# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

A Rust CLI tool that extracts word-by-word character spans from string literals in Rust source files. Given a Rust file and line number, it parses the string literal on that line and returns each word with its byte position within the string. The tool supports flexible filtering options to show only specific tokens/words that match exact patterns, contain substrings, or match regex patterns.

## Development Commands

### Building and Running
- Build the project: `cargo build`
- Run directly: `cargo run -- <file> <line_number>`
- Install locally: `cargo install --path .`

### Usage Examples

#### Basic Usage
```bash
# Extract spans from a string literal in a Rust file
cargo run -- file src/main.rs 42

# Process raw string content directly  
cargo run -- string "hello world test"

# Read from stdin
echo "hello world" | cargo run -- string
```

#### Filtering Options
```bash
# Filter to show only specific words (exact match)
cargo run -- --filter hello --filter world string "hello world test"

# Use contains mode to match substrings
cargo run -- --filter-mode contains --filter "orl" string "hello world wonderful"

# Use regex patterns for advanced filtering
cargo run -- --filter-mode regex --filter "w.*d" string "hello world test"

# Case-insensitive filtering
cargo run -- --filter HELLO --ignore-case string "hello world"

# Combine with strings-as-tokens mode
cargo run -- --strings-as-tokens --filter-mode contains --filter "quoted" string 'before "quoted text" after'
```

### Testing
- Run all tests: `cargo test`
- Run specific test: `cargo test <test_name>`
- Run with Nix: `nix develop` then standard cargo commands
- Build Nix package: `nix build`

## Architecture

### Core Components
- **main.rs**: Single-file implementation with all functionality
- **StringVisitor**: AST visitor that finds string literals on specific lines using syn's visitor pattern
- **WordSpan**: Data structure representing word boundaries with start/end positions
- **get_word_spans()**: Unicode-aware word boundary detection using unicode-segmentation crate

### Key Dependencies
- `syn`: Rust parser for AST traversal and string literal extraction
- `unicode-segmentation`: Proper word boundary detection for all Unicode text
- `clap`: Command-line argument parsing with derive features for structured CLI
- `proc-macro2`: Required for span location information
- `regex`: Pattern matching for regex-based filtering

### Filtering System
The tool includes a flexible filtering system that operates on extracted word spans:
- **Exact Mode**: Match words that exactly equal the filter strings
- **Contains Mode**: Match words that contain the filter substrings  
- **Regex Mode**: Match words using regular expression patterns
- **Case Sensitivity**: All modes support case-insensitive matching with `--ignore-case`
- **Multiple Filters**: Multiple filter patterns can be specified, with OR logic (any match includes the word)

### String Processing Logic
The tool handles various string literal types:
- Regular strings: `"hello world"`
- Raw strings: `r"hello world"`  
- Multiline strings spanning multiple lines
- Strings with escaped quotes and special characters

For multiline strings, any line number within the string's span returns the same complete word breakdown.

## Test Structure

### Test Files (test-files/)
- `simple.rs`: Basic single-line string literals
- `raw_string.rs`: Raw string literal examples
- `escaped.rs`: Strings with escaped quotes
- `multiline.rs`: Regular multiline string
- `multiline_raw.rs`: Raw multiline string

### Test Categories
- Unit tests for word boundary detection
- Integration tests using actual Rust source files
- Error condition testing (no strings, multiple strings)
- Unicode and punctuation handling tests

Tests verify both the string extraction from AST and word span calculation phases independently.