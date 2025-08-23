# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

A Rust CLI tool that extracts word-by-word character spans from string literals in Rust source files. Given a Rust file and line number, it parses the string literal on that line and returns each word with its byte position within the string.

## Development Commands

### Building and Running
- Build the project: `cargo build`
- Run directly: `cargo run -- <file> <line_number>`
- Install locally: `cargo install --path .`

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
- `clap`: Command-line argument parsing
- `proc-macro2`: Required for span location information

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