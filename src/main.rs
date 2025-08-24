use clap::{Parser, Subcommand, ValueEnum};
use regex::Regex;
use std::fs;
use std::io::{self, Read};
use std::path::PathBuf;
use syn::{visit::Visit, File, LitStr};
use unicode_segmentation::UnicodeSegmentation;

#[derive(Clone, Debug, ValueEnum)]
enum FilterMode {
    /// Exact word match
    Exact,
    /// Word contains the filter string
    Contains,
    /// Word matches the regex pattern
    Regex,
}

impl Default for FilterMode {
    fn default() -> Self {
        FilterMode::Exact
    }
}

/// Extract word-by-word character spans from string literals
#[derive(Parser)]
#[command(name = "rust-span-counter")]
#[command(about = "Extracts strings and provides word-by-word character spans")]
struct Args {
    /// Treat quoted strings as single tokens (preserving quote boundaries)
    #[arg(long, help = "Treat quoted content (\"...\", '...', `...`) as single tokens")]
    strings_as_tokens: bool,

    /// Filter output to include only specified words/tokens (can be used multiple times)
    #[arg(long = "filter", short = 'f', help = "Filter to include only specified words (can be used multiple times)")]
    filters: Vec<String>,

    /// Filter mode: exact, contains, or regex
    #[arg(long, value_enum, default_value_t = FilterMode::Exact, help = "Filter mode: exact match, contains, or regex pattern")]
    filter_mode: FilterMode,

    /// Case-insensitive filtering
    #[arg(long, help = "Case-insensitive filtering")]
    ignore_case: bool,

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
    RegexError(regex::Error),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::IoError(err) => write!(f, "File error: {}", err),
            Error::ParseError(err) => write!(f, "Parse error: {}", err),
            Error::NoStringFound => write!(f, "No string found on the specified line"),
            Error::MultipleStringsFound => write!(f, "Multiple strings found on the same line"),
            Error::RegexError(err) => write!(f, "Regex error: {}", err),
        }
    }
}

impl std::error::Error for Error {}

fn main() -> Result<(), Error> {
    let args = Args::parse();
    
    let string_content = match &args.command {
        Commands::File { file_path, line_number } => {
            handle_file_command(file_path, *line_number)?
        }
        Commands::String { content } => {
            handle_string_command(content.as_deref())?
        }
    };
    
    let spans = get_word_spans(&string_content, args.strings_as_tokens)?;
    let filtered_spans = filter_word_spans(spans, &args.filters, &args.filter_mode, args.ignore_case)?;
    
    // Print the results
    for span in filtered_spans {
        println!("{}", span);
    }
    
    Ok(())
}

fn filter_word_spans(spans: Vec<WordSpan>, filters: &[String], filter_mode: &FilterMode, ignore_case: bool) -> Result<Vec<WordSpan>, Error> {
    if filters.is_empty() {
        return Ok(spans);
    }

    match filter_mode {
        FilterMode::Exact => {
            let filtered = spans.into_iter()
                .filter(|span| {
                    filters.iter().any(|filter| {
                        if ignore_case {
                            span.word.to_lowercase() == filter.to_lowercase()
                        } else {
                            span.word == *filter
                        }
                    })
                })
                .collect();
            Ok(filtered)
        }
        FilterMode::Contains => {
            let filtered = spans.into_iter()
                .filter(|span| {
                    filters.iter().any(|filter| {
                        if ignore_case {
                            span.word.to_lowercase().contains(&filter.to_lowercase())
                        } else {
                            span.word.contains(filter)
                        }
                    })
                })
                .collect();
            Ok(filtered)
        }
        FilterMode::Regex => {
            let mut compiled_regexes = Vec::new();
            for filter in filters {
                let regex = if ignore_case {
                    Regex::new(&format!("(?i){}", filter)).map_err(Error::RegexError)?
                } else {
                    Regex::new(filter).map_err(Error::RegexError)?
                };
                compiled_regexes.push(regex);
            }
            
            let filtered = spans.into_iter()
                .filter(|span| {
                    compiled_regexes.iter().any(|regex| regex.is_match(&span.word))
                })
                .collect();
            Ok(filtered)
        }
    }
}

fn handle_file_command(file_path: &PathBuf, line_number: usize) -> Result<String, Error> {
    // Read and parse the file
    let content = fs::read_to_string(file_path).map_err(Error::IoError)?;
    let file = syn::parse_file(&content).map_err(Error::ParseError)?;
    
    // Find string literals on the target line and return the content
    find_strings_on_line(&file, line_number)
}

fn handle_string_command(content: Option<&str>) -> Result<String, Error> {
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
    
    Ok(input)
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

fn get_word_spans(string_content: &str, strings_as_tokens: bool) -> Result<Vec<WordSpan>, Error> {
    if strings_as_tokens {
        get_word_spans_with_quoted_strings(string_content)
    } else {
        get_word_spans_default(string_content)
    }
}

fn get_word_spans_default(string_content: &str) -> Result<Vec<WordSpan>, Error> {
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

fn get_word_spans_with_quoted_strings(string_content: &str) -> Result<Vec<WordSpan>, Error> {
    let mut spans = Vec::new();
    let chars: Vec<char> = string_content.chars().collect();
    let mut i = 0;
    
    while i < chars.len() {
        let ch = chars[i];
        
        // Check if we're starting a quoted string
        if ch == '"' || ch == '\'' || ch == '`' {
            let quote_char = ch;
            let quote_start = i;
            i += 1; // Move past opening quote
            
            // Find the matching closing quote, handling escapes
            while i < chars.len() {
                if chars[i] == '\\' && i + 1 < chars.len() {
                    // Skip escaped character
                    i += 2;
                } else if chars[i] == quote_char {
                    // Found closing quote
                    i += 1;
                    break;
                } else {
                    i += 1;
                }
            }
            
            // Create a span for the entire quoted string (including quotes)
            let byte_start: usize = chars[..quote_start].iter().map(|c| c.len_utf8()).sum();
            let byte_end: usize = chars[..i].iter().map(|c| c.len_utf8()).sum();
            let quoted_text: String = chars[quote_start..i].iter().collect();
            
            spans.push(WordSpan {
                word: quoted_text,
                start: byte_start,
                end: byte_end,
            });
        } else if ch.is_whitespace() {
            // Skip whitespace
            i += 1;
        } else {
            // Handle unquoted text - find the end of this token
            let token_start = i;
            
            while i < chars.len() {
                let current = chars[i];
                if current.is_whitespace() || current == '"' || current == '\'' || current == '`' {
                    break;
                }
                i += 1;
            }
            
            // Process this unquoted segment using word boundaries
            let byte_start: usize = chars[..token_start].iter().map(|c| c.len_utf8()).sum();
            let segment: String = chars[token_start..i].iter().collect();
            
            // Apply word boundary splitting to unquoted segments
            let mut segment_byte_pos = byte_start;
            for word_segment in segment.split_word_bounds() {
                if !word_segment.chars().all(|c| c.is_whitespace()) {
                    spans.push(WordSpan {
                        word: word_segment.to_string(),
                        start: segment_byte_pos,
                        end: segment_byte_pos + word_segment.len(),
                    });
                }
                segment_byte_pos += word_segment.len();
            }
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
        let spans = get_word_spans(content, false).unwrap();
        
        assert_eq!(spans, vec![
            WordSpan { word: "hello".to_string(), start: 0, end: 5 },
            WordSpan { word: "world".to_string(), start: 6, end: 11 }
        ]);
    }

    #[test]
    fn test_escaped_quotes() {
        let content = "foo bar";  // This simulates the parsed content of "foo \"bar"
        let spans = get_word_spans(content, false).unwrap();
        
        assert_eq!(spans, vec![
            WordSpan { word: "foo".to_string(), start: 0, end: 3 },
            WordSpan { word: "bar".to_string(), start: 4, end: 7 }
        ]);
    }

    #[test]
    fn test_single_word() {
        let content = "hello";
        let spans = get_word_spans(content, false).unwrap();
        
        assert_eq!(spans, vec![
            WordSpan { word: "hello".to_string(), start: 0, end: 5 }
        ]);
    }

    #[test]
    fn test_empty_string() {
        let content = "";
        let spans = get_word_spans(content, false).unwrap();
        
        assert_eq!(spans, vec![]);
    }

    #[test]
    fn test_multiple_spaces() {
        let content = "hello    world";
        let spans = get_word_spans(content, false).unwrap();
        
        assert_eq!(spans, vec![
            WordSpan { word: "hello".to_string(), start: 0, end: 5 },
            WordSpan { word: "world".to_string(), start: 9, end: 14 }
        ]);
    }

    #[test]
    fn test_leading_trailing_spaces() {
        let content = "  hello world  ";
        let spans = get_word_spans(content, false).unwrap();
        
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
        
        let content = handle_file_command(&test_file_path, 2).unwrap();
        let spans = get_word_spans(&content, false).unwrap();
        
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
        
        let content = handle_file_command(&test_file_path, 2).unwrap();
        let spans = get_word_spans(&content, false).unwrap();
        
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
        
        let content = handle_file_command(&test_file_path, 2).unwrap();
        let spans = get_word_spans(&content, false).unwrap();
        
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
        
        let content = handle_file_command(&test_file_path, 3).unwrap();
        let spans = get_word_spans(&content, false).unwrap();
        
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
            let content = handle_file_command(&test_file_path, line_number).unwrap();
            let spans = get_word_spans(&content, false).unwrap();
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
            let content = handle_file_command(&test_file_path, line_number).unwrap();
            let spans = get_word_spans(&content, false).unwrap();
            assert_eq!(spans, expected_spans, "Failed for raw string line {}", line_number);
        }
    }

    #[test]
    fn test_single_line_on_multiline_file() {
        let test_file_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("test-files")
            .join("multiline.rs");
        
        let content = handle_file_command(&test_file_path, 5).unwrap();
        let spans = get_word_spans(&content, false).unwrap();
        
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
        let spans = get_word_spans(content, false).unwrap();
        
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
        let spans = get_word_spans(content, false).unwrap();
        
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
        let spans = get_word_spans(content, false).unwrap();
        
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
        let spans = get_word_spans(content, false).unwrap();
        
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
        let content = handle_string_command(Some("hello world")).unwrap();
        let spans = get_word_spans(&content, false).unwrap();
        
        assert_eq!(spans, vec![
            WordSpan { word: "hello".to_string(), start: 0, end: 5 },
            WordSpan { word: "world".to_string(), start: 6, end: 11 }
        ]);
    }

    #[test]
    fn test_string_subcommand_empty_string() {
        let content = handle_string_command(Some("")).unwrap();
        let spans = get_word_spans(&content, false).unwrap();
        
        assert_eq!(spans, vec![]);
    }

    #[test]
    fn test_string_subcommand_punctuation() {
        let content = handle_string_command(Some("hello, world!")).unwrap();
        let spans = get_word_spans(&content, false).unwrap();
        
        assert_eq!(spans, vec![
            WordSpan { word: "hello".to_string(), start: 0, end: 5 },
            WordSpan { word: ",".to_string(), start: 5, end: 6 },
            WordSpan { word: "world".to_string(), start: 7, end: 12 },
            WordSpan { word: "!".to_string(), start: 12, end: 13 }
        ]);
    }

    #[test]
    fn test_string_subcommand_multiline_content() {
        let input = "hello\nworld\ntest";
        let content = handle_string_command(Some(input)).unwrap();
        let spans = get_word_spans(&content, false).unwrap();
        
        assert_eq!(spans, vec![
            WordSpan { word: "hello".to_string(), start: 0, end: 5 },
            WordSpan { word: "world".to_string(), start: 6, end: 11 },
            WordSpan { word: "test".to_string(), start: 12, end: 16 }
        ]);
    }

    // Tests for the new strings-as-tokens functionality
    #[test]
    fn test_strings_as_tokens_double_quotes() {
        let content = "hello \"world test\" end";
        let spans = get_word_spans(content, true).unwrap();
        
        assert_eq!(spans, vec![
            WordSpan { word: "hello".to_string(), start: 0, end: 5 },
            WordSpan { word: "\"world test\"".to_string(), start: 6, end: 18 },
            WordSpan { word: "end".to_string(), start: 19, end: 22 }
        ]);
    }

    #[test]
    fn test_strings_as_tokens_single_quotes() {
        let content = "hello 'world test' end";
        let spans = get_word_spans(content, true).unwrap();
        
        assert_eq!(spans, vec![
            WordSpan { word: "hello".to_string(), start: 0, end: 5 },
            WordSpan { word: "'world test'".to_string(), start: 6, end: 18 },
            WordSpan { word: "end".to_string(), start: 19, end: 22 }
        ]);
    }

    #[test]
    fn test_strings_as_tokens_backticks() {
        let content = "hello `world test` end";
        let spans = get_word_spans(content, true).unwrap();
        
        assert_eq!(spans, vec![
            WordSpan { word: "hello".to_string(), start: 0, end: 5 },
            WordSpan { word: "`world test`".to_string(), start: 6, end: 18 },
            WordSpan { word: "end".to_string(), start: 19, end: 22 }
        ]);
    }

    #[test]
    fn test_strings_as_tokens_mixed_quotes() {
        let content = "say \"hello\" and 'world' plus `test`";
        let spans = get_word_spans(content, true).unwrap();
        
        assert_eq!(spans, vec![
            WordSpan { word: "say".to_string(), start: 0, end: 3 },
            WordSpan { word: "\"hello\"".to_string(), start: 4, end: 11 },
            WordSpan { word: "and".to_string(), start: 12, end: 15 },
            WordSpan { word: "'world'".to_string(), start: 16, end: 23 },
            WordSpan { word: "plus".to_string(), start: 24, end: 28 },
            WordSpan { word: "`test`".to_string(), start: 29, end: 35 }
        ]);
    }

    #[test]
    fn test_strings_as_tokens_escaped_quotes() {
        let content = "before \"she said \\\"hello\\\" there\" after";
        let spans = get_word_spans(content, true).unwrap();
        
        assert_eq!(spans, vec![
            WordSpan { word: "before".to_string(), start: 0, end: 6 },
            WordSpan { word: "\"she said \\\"hello\\\" there\"".to_string(), start: 7, end: 33 },
            WordSpan { word: "after".to_string(), start: 34, end: 39 }
        ]);
    }

    #[test]
    fn test_strings_as_tokens_empty_quotes() {
        let content = "before \"\" empty '' and `` after";
        let spans = get_word_spans(content, true).unwrap();
        
        assert_eq!(spans, vec![
            WordSpan { word: "before".to_string(), start: 0, end: 6 },
            WordSpan { word: "\"\"".to_string(), start: 7, end: 9 },
            WordSpan { word: "empty".to_string(), start: 10, end: 15 },
            WordSpan { word: "''".to_string(), start: 16, end: 18 },
            WordSpan { word: "and".to_string(), start: 19, end: 22 },
            WordSpan { word: "``".to_string(), start: 23, end: 25 },
            WordSpan { word: "after".to_string(), start: 26, end: 31 }
        ]);
    }

    #[test]
    fn test_strings_as_tokens_unclosed_quotes() {
        let content = "hello \"unclosed quote and more";
        let spans = get_word_spans(content, true).unwrap();
        
        // Unclosed quotes should consume the rest of the string
        assert_eq!(spans, vec![
            WordSpan { word: "hello".to_string(), start: 0, end: 5 },
            WordSpan { word: "\"unclosed quote and more".to_string(), start: 6, end: 30 }
        ]);
    }

    #[test]
    fn test_strings_as_tokens_vs_default_comparison() {
        let content = "hello 'world test' end";
        
        // Default behavior
        let default_spans = get_word_spans(content, false).unwrap();
        assert_eq!(default_spans, vec![
            WordSpan { word: "hello".to_string(), start: 0, end: 5 },
            WordSpan { word: "'".to_string(), start: 6, end: 7 },
            WordSpan { word: "world".to_string(), start: 7, end: 12 },
            WordSpan { word: "test".to_string(), start: 13, end: 17 },
            WordSpan { word: "'".to_string(), start: 17, end: 18 },
            WordSpan { word: "end".to_string(), start: 19, end: 22 }
        ]);
        
        // Strings-as-tokens behavior
        let token_spans = get_word_spans(content, true).unwrap();
        assert_eq!(token_spans, vec![
            WordSpan { word: "hello".to_string(), start: 0, end: 5 },
            WordSpan { word: "'world test'".to_string(), start: 6, end: 18 },
            WordSpan { word: "end".to_string(), start: 19, end: 22 }
        ]);
    }

    #[test]
    fn test_strings_as_tokens_unquoted_punctuation() {
        let content = "array[index] \"quoted text\" + value*2";
        let spans = get_word_spans(content, true).unwrap();
        
        assert_eq!(spans, vec![
            WordSpan { word: "array".to_string(), start: 0, end: 5 },
            WordSpan { word: "[".to_string(), start: 5, end: 6 },
            WordSpan { word: "index".to_string(), start: 6, end: 11 },
            WordSpan { word: "]".to_string(), start: 11, end: 12 },
            WordSpan { word: "\"quoted text\"".to_string(), start: 13, end: 26 },
            WordSpan { word: "+".to_string(), start: 27, end: 28 },
            WordSpan { word: "value".to_string(), start: 29, end: 34 },
            WordSpan { word: "*".to_string(), start: 34, end: 35 },
            WordSpan { word: "2".to_string(), start: 35, end: 36 }
        ]);
    }

    // Tests for filtering functionality
    #[test]
    fn test_filter_exact_match() {
        let spans = vec![
            WordSpan { word: "hello".to_string(), start: 0, end: 5 },
            WordSpan { word: "world".to_string(), start: 6, end: 11 },
            WordSpan { word: "test".to_string(), start: 12, end: 16 }
        ];
        
        let filters = vec!["world".to_string()];
        let result = filter_word_spans(spans, &filters, &FilterMode::Exact, false).unwrap();
        
        assert_eq!(result, vec![
            WordSpan { word: "world".to_string(), start: 6, end: 11 }
        ]);
    }
    
    #[test]
    fn test_filter_exact_match_multiple() {
        let spans = vec![
            WordSpan { word: "hello".to_string(), start: 0, end: 5 },
            WordSpan { word: "world".to_string(), start: 6, end: 11 },
            WordSpan { word: "test".to_string(), start: 12, end: 16 }
        ];
        
        let filters = vec!["hello".to_string(), "test".to_string()];
        let result = filter_word_spans(spans, &filters, &FilterMode::Exact, false).unwrap();
        
        assert_eq!(result, vec![
            WordSpan { word: "hello".to_string(), start: 0, end: 5 },
            WordSpan { word: "test".to_string(), start: 12, end: 16 }
        ]);
    }
    
    #[test]
    fn test_filter_exact_match_case_sensitive() {
        let spans = vec![
            WordSpan { word: "Hello".to_string(), start: 0, end: 5 },
            WordSpan { word: "WORLD".to_string(), start: 6, end: 11 },
        ];
        
        let filters = vec!["hello".to_string()];
        let result = filter_word_spans(spans, &filters, &FilterMode::Exact, false).unwrap();
        
        assert_eq!(result, vec![]); // No matches because of case sensitivity
    }
    
    #[test]
    fn test_filter_exact_match_case_insensitive() {
        let spans = vec![
            WordSpan { word: "Hello".to_string(), start: 0, end: 5 },
            WordSpan { word: "WORLD".to_string(), start: 6, end: 11 },
        ];
        
        let filters = vec!["hello".to_string(), "world".to_string()];
        let result = filter_word_spans(spans, &filters, &FilterMode::Exact, true).unwrap();
        
        assert_eq!(result, vec![
            WordSpan { word: "Hello".to_string(), start: 0, end: 5 },
            WordSpan { word: "WORLD".to_string(), start: 6, end: 11 }
        ]);
    }
    
    #[test]
    fn test_filter_contains_mode() {
        let spans = vec![
            WordSpan { word: "hello".to_string(), start: 0, end: 5 },
            WordSpan { word: "world".to_string(), start: 6, end: 11 },
            WordSpan { word: "wonderful".to_string(), start: 12, end: 21 }
        ];
        
        let filters = vec!["orl".to_string(), "nde".to_string()];
        let result = filter_word_spans(spans, &filters, &FilterMode::Contains, false).unwrap();
        
        assert_eq!(result, vec![
            WordSpan { word: "world".to_string(), start: 6, end: 11 },
            WordSpan { word: "wonderful".to_string(), start: 12, end: 21 }
        ]);
    }
    
    #[test]
    fn test_filter_contains_case_insensitive() {
        let spans = vec![
            WordSpan { word: "Hello".to_string(), start: 0, end: 5 },
            WordSpan { word: "WORLD".to_string(), start: 6, end: 11 },
        ];
        
        let filters = vec!["ell".to_string(), "orl".to_string()];
        let result = filter_word_spans(spans, &filters, &FilterMode::Contains, true).unwrap();
        
        assert_eq!(result, vec![
            WordSpan { word: "Hello".to_string(), start: 0, end: 5 },
            WordSpan { word: "WORLD".to_string(), start: 6, end: 11 }
        ]);
    }
    
    #[test]
    fn test_filter_regex_mode() {
        let spans = vec![
            WordSpan { word: "hello".to_string(), start: 0, end: 5 },
            WordSpan { word: "world".to_string(), start: 6, end: 11 },
            WordSpan { word: "word".to_string(), start: 12, end: 16 },
            WordSpan { word: "test123".to_string(), start: 17, end: 24 }
        ];
        
        let filters = vec![r"wo.*d".to_string()];
        let result = filter_word_spans(spans, &filters, &FilterMode::Regex, false).unwrap();
        
        assert_eq!(result, vec![
            WordSpan { word: "world".to_string(), start: 6, end: 11 },
            WordSpan { word: "word".to_string(), start: 12, end: 16 }
        ]);
    }
    
    #[test]
    fn test_filter_regex_with_numbers() {
        let spans = vec![
            WordSpan { word: "test123".to_string(), start: 0, end: 7 },
            WordSpan { word: "hello".to_string(), start: 8, end: 13 },
            WordSpan { word: "world456".to_string(), start: 14, end: 22 }
        ];
        
        let filters = vec![r"\d+".to_string()]; // Match words containing digits
        let result = filter_word_spans(spans, &filters, &FilterMode::Regex, false).unwrap();
        
        assert_eq!(result, vec![
            WordSpan { word: "test123".to_string(), start: 0, end: 7 },
            WordSpan { word: "world456".to_string(), start: 14, end: 22 }
        ]);
    }
    
    #[test]
    fn test_filter_regex_case_insensitive() {
        let spans = vec![
            WordSpan { word: "Hello".to_string(), start: 0, end: 5 },
            WordSpan { word: "WORLD".to_string(), start: 6, end: 11 },
        ];
        
        let filters = vec!["hello".to_string()];
        let result = filter_word_spans(spans, &filters, &FilterMode::Regex, true).unwrap();
        
        assert_eq!(result, vec![
            WordSpan { word: "Hello".to_string(), start: 0, end: 5 }
        ]);
    }
    
    #[test]
    fn test_filter_invalid_regex() {
        let spans = vec![
            WordSpan { word: "hello".to_string(), start: 0, end: 5 }
        ];
        
        let filters = vec!["[invalid".to_string()]; // Invalid regex
        let result = filter_word_spans(spans, &filters, &FilterMode::Regex, false);
        
        assert!(matches!(result, Err(Error::RegexError(_))));
    }
    
    #[test]
    fn test_filter_empty_filters() {
        let spans = vec![
            WordSpan { word: "hello".to_string(), start: 0, end: 5 },
            WordSpan { word: "world".to_string(), start: 6, end: 11 }
        ];
        
        let filters = vec![];
        let result = filter_word_spans(spans.clone(), &filters, &FilterMode::Exact, false).unwrap();
        
        assert_eq!(result, spans); // Should return all spans when no filters
    }
    
    #[test]
    fn test_filter_no_matches() {
        let spans = vec![
            WordSpan { word: "hello".to_string(), start: 0, end: 5 },
            WordSpan { word: "world".to_string(), start: 6, end: 11 }
        ];
        
        let filters = vec!["nonexistent".to_string()];
        let result = filter_word_spans(spans, &filters, &FilterMode::Exact, false).unwrap();
        
        assert_eq!(result, vec![]); // Should return empty vec when no matches
    }
    
    #[test]
    fn test_filter_with_punctuation() {
        let spans = vec![
            WordSpan { word: "hello".to_string(), start: 0, end: 5 },
            WordSpan { word: ",".to_string(), start: 5, end: 6 },
            WordSpan { word: "world".to_string(), start: 7, end: 12 },
            WordSpan { word: "!".to_string(), start: 12, end: 13 }
        ];
        
        let filters = vec![",".to_string(), "!".to_string()];
        let result = filter_word_spans(spans, &filters, &FilterMode::Exact, false).unwrap();
        
        assert_eq!(result, vec![
            WordSpan { word: ",".to_string(), start: 5, end: 6 },
            WordSpan { word: "!".to_string(), start: 12, end: 13 }
        ]);
    }
}