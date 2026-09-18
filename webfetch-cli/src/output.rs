use crate::fetch::{FetchResult, Sliced};

/// Human-readable output.
pub fn print_human(result: &FetchResult, title: &Option<String>, mode: &str, sliced: &Sliced, max_length: usize) {
    println!("URL: {}", result.url);
    println!("Final URL: {}", result.final_url);
    println!("Type: {}", result.content_type);
    if let Some(title) = title {
        println!("Title: {}", title);
    }
    println!("Mode: {}", mode);
    println!("---");
    println!("{}", sliced.text);
    println!("---");
    if sliced.truncated {
        if let Some(next) = sliced.next_start_index {
            println!("(truncated at {} chars, next: {})", max_length, next);
        }
    }
}

/// JSON output (matches the MCP server's tool return value exactly).
pub fn print_json(value: &serde_json::Value) {
    println!("{}", serde_json::to_string_pretty(value).unwrap());
}
