use clap::Parser;
use std::fs;
use std::path::PathBuf;
use syn::{visit::Visit, File, LitStr};

/// Extract word-by-word character spans from string literals in Rust source files
///
/// Example: For line `let s = "hello world";` outputs:
/// "hello" | 0-5
/// "world" | 6-11
#[derive(Parser)]
#[command(name = "rust-span-counter")]
#[command(about = "Extracts strings from Rust files and provides word-by-word character spans")]
struct Args {
    /// Path to the Rust source file (.rs)
    #[arg(value_name = "FILE")]
    file_path: PathBuf,
    
    /// Line number containing the string literal (1-based)
    #[arg(value_name = "LINE_NUM")]
    line_number: usize,
}

#[derive(Debug)]
enum Error {
    IoError(std::io::Error),
    ParseError(syn::Error),
    NoStringFound,
    MultipleStringsFound,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::IoError(err) => write!(f, "File error: {}", err),
            Error::ParseError(err) => write!(f, "Parse error: {}", err),
            Error::NoStringFound => write!(f, "No string found on the specified line"),
            Error::MultipleStringsFound => write!(f, "Multiple strings found on the same line"),
        }
    }
}

impl std::error::Error for Error {}

fn main() -> Result<(), Error> {
    let args = Args::parse();
    let spans = process_file(&args)?;
    
    // Print the results
    for span in spans {
        println!("{}", span);
    }
    
    Ok(())
}

fn process_file(args: &Args) -> Result<Vec<WordSpan>, Error> {
    // Read and parse the file
    let content = fs::read_to_string(&args.file_path).map_err(Error::IoError)?;
    let file = syn::parse_file(&content).map_err(Error::ParseError)?;
    
    // Find string literals on the target line
    let string_literals = find_strings_on_line(&file, args.line_number)?;
    
    // Process the string and return word spans
    get_word_spans(&string_literals)
}

fn find_strings_on_line(file: &File, target_line: usize) -> Result<String, Error> {
    let mut visitor = StringVisitor::new(target_line);
    visitor.visit_file(file);
    
    match visitor.found_strings.len() {
        0 => Err(Error::NoStringFound),
        1 => Ok(visitor.found_strings.into_iter().next().unwrap()),
        _ => Err(Error::MultipleStringsFound),
    }
}

struct StringVisitor {
    target_line: usize,
    found_strings: Vec<String>,
}

impl StringVisitor {
    fn new(target_line: usize) -> Self {
        Self {
            target_line,
            found_strings: Vec::new(),
        }
    }
}

impl<'ast> Visit<'ast> for StringVisitor {
    fn visit_lit_str(&mut self, lit_str: &'ast LitStr) {
        let span = lit_str.span();
        let start_line = span.start().line;
        let end_line = span.end().line;
        
        if self.target_line >= start_line && self.target_line <= end_line {
            self.found_strings.push(lit_str.value());
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct WordSpan {
    pub word: String,
    pub start: usize,
    pub end: usize,
}

impl std::fmt::Display for WordSpan {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "\"{}\" | {}-{}", self.word, self.start, self.end)
    }
}

fn get_word_spans(string_content: &str) -> Result<Vec<WordSpan>, Error> {
    let words: Vec<&str> = string_content.split_whitespace().collect();
    let mut spans = Vec::new();
    let mut current_pos = 0;
    
    for word in words {
        // Find the word in the remaining string to handle multiple spaces correctly
        if let Some(word_start) = string_content[current_pos..].find(word) {
            let absolute_start = current_pos + word_start;
            let absolute_end = absolute_start + word.len();
            
            spans.push(WordSpan {
                word: word.to_string(),
                start: absolute_start,
                end: absolute_end,
            });
            
            current_pos = absolute_end;
        }
    }
    
    Ok(spans)
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_word_splitting() {
        let content = "hello world";
        let spans = get_word_spans(content).unwrap();
        
        assert_eq!(spans, vec![
            WordSpan { word: "hello".to_string(), start: 0, end: 5 },
            WordSpan { word: "world".to_string(), start: 6, end: 11 }
        ]);
    }

    #[test]
    fn test_escaped_quotes() {
        let content = "foo bar";  // This simulates the parsed content of "foo \"bar"
        let spans = get_word_spans(content).unwrap();
        
        assert_eq!(spans, vec![
            WordSpan { word: "foo".to_string(), start: 0, end: 3 },
            WordSpan { word: "bar".to_string(), start: 4, end: 7 }
        ]);
    }

    #[test]
    fn test_single_word() {
        let content = "hello";
        let spans = get_word_spans(content).unwrap();
        
        assert_eq!(spans, vec![
            WordSpan { word: "hello".to_string(), start: 0, end: 5 }
        ]);
    }

    #[test]
    fn test_empty_string() {
        let content = "";
        let spans = get_word_spans(content).unwrap();
        
        assert_eq!(spans, vec![]);
    }

    #[test]
    fn test_multiple_spaces() {
        let content = "hello    world";
        let spans = get_word_spans(content).unwrap();
        
        assert_eq!(spans, vec![
            WordSpan { word: "hello".to_string(), start: 0, end: 5 },
            WordSpan { word: "world".to_string(), start: 9, end: 14 }
        ]);
    }

    #[test]
    fn test_leading_trailing_spaces() {
        let content = "  hello world  ";
        let spans = get_word_spans(content).unwrap();
        
        assert_eq!(spans, vec![
            WordSpan { word: "hello".to_string(), start: 2, end: 7 },
            WordSpan { word: "world".to_string(), start: 8, end: 13 }
        ]);
    }

    #[test]
    fn test_string_extraction_from_line() {
        let code = r#"
        fn main() {
            let s = "hello world";
        }
        "#;
        
        let file = syn::parse_file(code).unwrap();
        let result = find_strings_on_line(&file, 3);
        
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "hello world");
    }

    #[test]
    fn test_no_string_on_line() {
        let code = r#"
        fn main() {
            let x = 42;
        }
        "#;
        
        let file = syn::parse_file(code).unwrap();
        let result = find_strings_on_line(&file, 3);
        
        assert!(matches!(result, Err(Error::NoStringFound)));
    }

    #[test]
    fn test_multiple_strings_error() {
        let code = r#"
        fn main() {
            let s = "hello"; let t = "world";
        }
        "#;
        
        let file = syn::parse_file(code).unwrap();
        let result = find_strings_on_line(&file, 3);
        
        assert!(matches!(result, Err(Error::MultipleStringsFound)));
    }

    #[test]
    fn test_raw_string() {
        let code = r#"
        fn main() {
            let s = r"hello world";
        }
        "#;
        
        let file = syn::parse_file(code).unwrap();
        let result = find_strings_on_line(&file, 3);
        
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "hello world");
    }

    #[test]
    fn test_string_with_escapes() {
        let code = r#"let s = "foo \"bar\" baz";"#;
        let file = syn::parse_str::<syn::Stmt>(code).unwrap();
        
        let mut visitor = StringVisitor::new(1);
        visitor.visit_stmt(&file);
        
        assert_eq!(visitor.found_strings.len(), 1);
        assert_eq!(visitor.found_strings[0], "foo \"bar\" baz");
    }

    #[test]
    fn test_complete_workflow() {
        let test_file_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("test-files")
            .join("simple.rs");
        
        let args = Args {
            file_path: test_file_path,
            line_number: 2,
        };
        
        let spans = process_file(&args).unwrap();
        
        assert_eq!(spans, vec![
            WordSpan { word: "hello".to_string(), start: 0, end: 5 },
            WordSpan { word: "world".to_string(), start: 6, end: 11 },
            WordSpan { word: "test".to_string(), start: 12, end: 16 }
        ]);
    }

    #[test]
    fn test_complete_workflow_with_raw_string() {
        let test_file_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("test-files")
            .join("raw_string.rs");
        
        let args = Args {
            file_path: test_file_path,
            line_number: 2,
        };
        
        let spans = process_file(&args).unwrap();
        
        assert_eq!(spans, vec![
            WordSpan { word: "raw".to_string(), start: 0, end: 3 },
            WordSpan { word: "string".to_string(), start: 4, end: 10 },
            WordSpan { word: "content".to_string(), start: 11, end: 18 }
        ]);
    }

    #[test]
    fn test_complete_workflow_with_escaped_quotes() {
        let test_file_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("test-files")
            .join("escaped.rs");
        
        let args = Args {
            file_path: test_file_path,
            line_number: 2,
        };
        
        let spans = process_file(&args).unwrap();
        
        assert_eq!(spans, vec![
            WordSpan { word: "foo".to_string(), start: 0, end: 3 },
            WordSpan { word: "\"bar\"".to_string(), start: 4, end: 9 },
            WordSpan { word: "baz".to_string(), start: 10, end: 13 }
        ]);
    }

    #[test] 
    fn test_complete_workflow_line_3() {
        let test_file_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("test-files")
            .join("simple.rs");
        
        let args = Args {
            file_path: test_file_path,
            line_number: 3,
        };
        
        let spans = process_file(&args).unwrap();
        
        assert_eq!(spans, vec![
            WordSpan { word: "foo".to_string(), start: 0, end: 3 },
            WordSpan { word: "bar".to_string(), start: 4, end: 7 },
            WordSpan { word: "baz".to_string(), start: 8, end: 11 }
        ]);
    }

    #[test]
    fn test_multiline_string_multiple_lines() {
        let test_file_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("test-files")
            .join("multiline.rs");
        
        let expected_spans = vec![
            WordSpan { word: "this".to_string(), start: 0, end: 4 },
            WordSpan { word: "is".to_string(), start: 5, end: 7 },
            WordSpan { word: "a".to_string(), start: 8, end: 9 },
            WordSpan { word: "multiline".to_string(), start: 23, end: 32 },
            WordSpan { word: "string".to_string(), start: 33, end: 39 },
            WordSpan { word: "with".to_string(), start: 40, end: 44 },
            WordSpan { word: "multiple".to_string(), start: 58, end: 66 },
            WordSpan { word: "words".to_string(), start: 67, end: 72 },
            WordSpan { word: "per".to_string(), start: 73, end: 76 },
            WordSpan { word: "line".to_string(), start: 77, end: 81 }
        ];

        // Test that all lines covered by the multiline string return the same result
        for line_number in [2, 3, 4] {
            let args = Args {
                file_path: test_file_path.clone(),
                line_number,
            };
            
            let spans = process_file(&args).unwrap();
            assert_eq!(spans, expected_spans, "Failed for line {}", line_number);
        }
    }

    #[test]
    fn test_multiline_raw_string_multiple_lines() {
        let test_file_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("test-files")
            .join("multiline_raw.rs");
        
        let expected_spans = vec![
            WordSpan { word: "this".to_string(), start: 0, end: 4 },
            WordSpan { word: "is".to_string(), start: 5, end: 7 },
            WordSpan { word: "a".to_string(), start: 8, end: 9 },
            WordSpan { word: "raw".to_string(), start: 10, end: 13 },
            WordSpan { word: "multiline".to_string(), start: 29, end: 38 },
            WordSpan { word: "string".to_string(), start: 39, end: 45 },
            WordSpan { word: "with".to_string(), start: 46, end: 50 },
            WordSpan { word: "special".to_string(), start: 66, end: 73 },
            WordSpan { word: "\"quotes\"".to_string(), start: 74, end: 82 },
            WordSpan { word: "and".to_string(), start: 83, end: 86 },
            WordSpan { word: "symbols".to_string(), start: 87, end: 94 }
        ];

        // Test that all lines covered by the multiline raw string return the same result
        for line_number in [2, 3, 4] {
            let args = Args {
                file_path: test_file_path.clone(),
                line_number,
            };
            
            let spans = process_file(&args).unwrap();
            assert_eq!(spans, expected_spans, "Failed for raw string line {}", line_number);
        }
    }

    #[test]
    fn test_single_line_on_multiline_file() {
        let test_file_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("test-files")
            .join("multiline.rs");
        
        let args = Args {
            file_path: test_file_path,
            line_number: 5,
        };
        
        let spans = process_file(&args).unwrap();
        
        // Should find the single line string on line 5
        assert_eq!(spans, vec![
            WordSpan { word: "single".to_string(), start: 0, end: 6 },
            WordSpan { word: "line".to_string(), start: 7, end: 11 },
            WordSpan { word: "string".to_string(), start: 12, end: 18 }
        ]);
    }

    #[test]
    fn test_no_string_on_multiline_boundary() {
        let test_file_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("test-files")
            .join("multiline.rs");
        
        let args = Args {
            file_path: test_file_path,
            line_number: 1,
        };
        
        let result = process_file(&args);
        
        // Should return NoStringFound error for line 1 (fn main() line)
        assert!(matches!(result, Err(Error::NoStringFound)));
    }
}