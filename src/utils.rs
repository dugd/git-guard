/// gives a rough estimate of tokens
pub fn estimate_tokens(text: &str) -> usize {
    if text.is_empty() { 0 } else { text.len() / 3 }
}
