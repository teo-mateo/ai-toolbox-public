use crate::search::SearchResult;

/// Print human-readable output (matches websearch-client.sh format).
pub fn print_human(result: &SearchResult) {
    println!("Query: {}", result.query);
    println!("Results: {}", result.results.len());
    println!();
    for (i, hit) in result.results.iter().enumerate() {
        println!("{}. {}", i + 1, hit.title);
        println!("   {}", hit.url);
        println!("   {}", hit.snippet);
        if i + 1 < result.results.len() {
            println!();
        }
    }
}

/// Print JSON output (matches MCP server's SearchResult.to_dict()).
pub fn print_json(result: &SearchResult) {
    let output = serde_json::json!({
        "query": result.query,
        "results": result.results.iter().map(|h| {
            serde_json::json!({
                "title": h.title,
                "url": h.url,
                "snippet": h.snippet,
            })
        }).collect::<Vec<_>>(),
    });
    println!("{}", serde_json::to_string_pretty(&output).unwrap());
}
