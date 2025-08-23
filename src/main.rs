use clap::{Parser, Subcommand};
use std::fs;
use std::io::{self, Read};
use std::path::PathBuf;
use syn::{visit::Visit, File, LitStr};
use unicode_segmentation::UnicodeSegmentation;

/// Extract word-by-word character spans from string literals
#[derive(Parser)]
#[command(name = "rust-span-counter")]
#[command(about = "Extracts strings and provides word-by-word character spans")]
struct Args {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Extract spans from a string literal in a Rust source file
    File {
        /// Path to the Rust source file (.rs)
        #[arg(value_name = "FILE")]
        file_path: PathBuf,
        
        /// Line number containing the string literal (1-based)
        #[arg(value_name = "LINE_NUM")]
        line_number: usize,
    },
    /// Extract spans from raw string content
    String {
        /// String content to process, or use "--" to read from stdin
        #[arg(value_name = "CONTENT")]
        content: Option<String>,
    },
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
    
    let spans = match &args.command {
        Commands::File { file_path, line_number } => {
            handle_file_command(file_path, *line_number)?
        }
        Commands::String { content } => {
            handle_string_command(content.as_deref())?
        }
    };
    
    // Print the results
    for span in spans {
        println!("{}", span);
    }
    
    Ok(())
}

fn handle_file_command(file_path: &PathBuf, line_number: usize) -> Result<Vec<WordSpan>, Error> {
    // Read and parse the file
    let content = fs::read_to_string(file_path).map_err(Error::IoError)?;
    let file = syn::parse_file(&content).map_err(Error::ParseError)?;
    
    // Find string literals on the target line
    let string_literals = find_strings_on_line(&file, line_number)?;
    
    // Process the string and return word spans
    get_word_spans(&string_literals)
}

fn handle_string_command(content: Option<&str>) -> Result<Vec<WordSpan>, Error> {
    let input = match content {
        Some("--") => {
            // Read from stdin
            read_from_stdin()?
        }
        Some(content) => content.to_string(),
        None => {
            // No content provided, read from stdin
            read_from_stdin()?
        }
    };
    
    get_word_spans(&input)
}

fn read_from_stdin() -> Result<String, Error> {
    let mut buffer = String::new();
    io::stdin().read_to_string(&mut buffer).map_err(Error::IoError)?;
    Ok(buffer)
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
    let mut spans = Vec::new();
    let mut byte_pos = 0;
    
    for segment in string_content.split_word_bounds() {
        // Only include non-whitespace segments as tokens
        if !segment.chars().all(|c| c.is_whitespace()) {
            spans.push(WordSpan {
                word: segment.to_string(),
                start: byte_pos,
                end: byte_pos + segment.len(),
            });
        }
        byte_pos += segment.len();
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
        
        let spans = handle_file_command(&test_file_path, 2).unwrap();
        
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
        
        let spans = handle_file_command(&test_file_path, 2).unwrap();
        
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
        
        let spans = handle_file_command(&test_file_path, 2).unwrap();
        
        assert_eq!(spans, vec![
            WordSpan { word: "foo".to_string(), start: 0, end: 3 },
            WordSpan { word: "\"".to_string(), start: 4, end: 5 },
            WordSpan { word: "bar".to_string(), start: 5, end: 8 },
            WordSpan { word: "\"".to_string(), start: 8, end: 9 },
            WordSpan { word: "baz".to_string(), start: 10, end: 13 }
        ]);
    }

    #[test] 
    fn test_complete_workflow_line_3() {
        let test_file_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("test-files")
            .join("simple.rs");
        
        let spans = handle_file_command(&test_file_path, 3).unwrap();
        
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
            let spans = handle_file_command(&test_file_path, line_number).unwrap();
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
            WordSpan { word: "\"".to_string(), start: 74, end: 75 },
            WordSpan { word: "quotes".to_string(), start: 75, end: 81 },
            WordSpan { word: "\"".to_string(), start: 81, end: 82 },
            WordSpan { word: "and".to_string(), start: 83, end: 86 },
            WordSpan { word: "symbols".to_string(), start: 87, end: 94 }
        ];

        // Test that all lines covered by the multiline raw string return the same result
        for line_number in [2, 3, 4] {
            let spans = handle_file_command(&test_file_path, line_number).unwrap();
            assert_eq!(spans, expected_spans, "Failed for raw string line {}", line_number);
        }
    }

    #[test]
    fn test_single_line_on_multiline_file() {
        let test_file_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("test-files")
            .join("multiline.rs");
        
        let spans = handle_file_command(&test_file_path, 5).unwrap();
        
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
        
        let result = handle_file_command(&test_file_path, 1);
        
        // Should return NoStringFound error for line 1 (fn main() line)
        assert!(matches!(result, Err(Error::NoStringFound)));
    }

    #[test]
    fn test_punctuation_tokenization() {
        let content = "default(nextval(user_id_seq)),";
        let spans = get_word_spans(content).unwrap();
        
        assert_eq!(spans, vec![
            WordSpan { word: "default".to_string(), start: 0, end: 7 },
            WordSpan { word: "(".to_string(), start: 7, end: 8 },
            WordSpan { word: "nextval".to_string(), start: 8, end: 15 },
            WordSpan { word: "(".to_string(), start: 15, end: 16 },
            WordSpan { word: "user_id_seq".to_string(), start: 16, end: 27 },
            WordSpan { word: ")".to_string(), start: 27, end: 28 },
            WordSpan { word: ")".to_string(), start: 28, end: 29 },
            WordSpan { word: ",".to_string(), start: 29, end: 30 },
        ]);
    }

    #[test]
    fn test_mixed_punctuation_and_whitespace() {
        let content = "hello, world! how are you?";
        let spans = get_word_spans(content).unwrap();
        
        assert_eq!(spans, vec![
            WordSpan { word: "hello".to_string(), start: 0, end: 5 },
            WordSpan { word: ",".to_string(), start: 5, end: 6 },
            WordSpan { word: "world".to_string(), start: 7, end: 12 },
            WordSpan { word: "!".to_string(), start: 12, end: 13 },
            WordSpan { word: "how".to_string(), start: 14, end: 17 },
            WordSpan { word: "are".to_string(), start: 18, end: 21 },
            WordSpan { word: "you".to_string(), start: 22, end: 25 },
            WordSpan { word: "?".to_string(), start: 25, end: 26 },
        ]);
    }

    #[test]
    fn test_sql_like_expression() {
        let content = "SELECT * FROM table WHERE id=42;";
        let spans = get_word_spans(content).unwrap();
        
        assert_eq!(spans, vec![
            WordSpan { word: "SELECT".to_string(), start: 0, end: 6 },
            WordSpan { word: "*".to_string(), start: 7, end: 8 },
            WordSpan { word: "FROM".to_string(), start: 9, end: 13 },
            WordSpan { word: "table".to_string(), start: 14, end: 19 },
            WordSpan { word: "WHERE".to_string(), start: 20, end: 25 },
            WordSpan { word: "id".to_string(), start: 26, end: 28 },
            WordSpan { word: "=".to_string(), start: 28, end: 29 },
            WordSpan { word: "42".to_string(), start: 29, end: 31 },
            WordSpan { word: ";".to_string(), start: 31, end: 32 },
        ]);
    }

    #[test]
    fn test_brackets_and_operators() {
        let content = "array[index]+value*2";
        let spans = get_word_spans(content).unwrap();
        
        assert_eq!(spans, vec![
            WordSpan { word: "array".to_string(), start: 0, end: 5 },
            WordSpan { word: "[".to_string(), start: 5, end: 6 },
            WordSpan { word: "index".to_string(), start: 6, end: 11 },
            WordSpan { word: "]".to_string(), start: 11, end: 12 },
            WordSpan { word: "+".to_string(), start: 12, end: 13 },
            WordSpan { word: "value".to_string(), start: 13, end: 18 },
            WordSpan { word: "*".to_string(), start: 18, end: 19 },
            WordSpan { word: "2".to_string(), start: 19, end: 20 },
        ]);
    }

    #[test]
    fn test_string_subcommand_with_content() {
        let spans = handle_string_command(Some("hello world")).unwrap();
        
        assert_eq!(spans, vec![
            WordSpan { word: "hello".to_string(), start: 0, end: 5 },
            WordSpan { word: "world".to_string(), start: 6, end: 11 }
        ]);
    }

    #[test]
    fn test_string_subcommand_empty_string() {
        let spans = handle_string_command(Some("")).unwrap();
        
        assert_eq!(spans, vec![]);
    }

    #[test]
    fn test_string_subcommand_punctuation() {
        let spans = handle_string_command(Some("hello, world!")).unwrap();
        
        assert_eq!(spans, vec![
            WordSpan { word: "hello".to_string(), start: 0, end: 5 },
            WordSpan { word: ",".to_string(), start: 5, end: 6 },
            WordSpan { word: "world".to_string(), start: 7, end: 12 },
            WordSpan { word: "!".to_string(), start: 12, end: 13 }
        ]);
    }

    #[test]
    fn test_string_subcommand_multiline_content() {
        let content = "hello\nworld\ntest";
        let spans = handle_string_command(Some(content)).unwrap();
        
        assert_eq!(spans, vec![
            WordSpan { word: "hello".to_string(), start: 0, end: 5 },
            WordSpan { word: "world".to_string(), start: 6, end: 11 },
            WordSpan { word: "test".to_string(), start: 12, end: 16 }
        ]);
    }
}