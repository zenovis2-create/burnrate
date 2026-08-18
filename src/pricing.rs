/// Per-million-token prices (USD). Hardcoded for v0; a `prices pull`
/// subcommand will replace this with an editable YAML later.

pub struct Price {
    pub input: f64,
    pub cache_write: f64,
    pub cache_read: f64,
    pub output: f64,
}

pub fn price_for(model: &str) -> Option<Price> {
    let m = model.to_lowercase();
    if m.contains("opus") {
        Some(Price { input: 5.0, cache_write: 6.25, cache_read: 0.5, output: 25.0 })
    } else if m.contains("sonnet") {
        Some(Price { input: 3.0, cache_write: 3.75, cache_read: 0.3, output: 15.0 })
    } else if m.contains("haiku") {
        Some(Price { input: 0.8, cache_write: 1.0, cache_read: 0.08, output: 4.0 })
    } else if m.contains("codex") || m.starts_with("gpt-5") {
        Some(Price { input: 1.25, cache_write: 0.0, cache_read: 0.125, output: 10.0 })
    } else if m.starts_with("gpt-") {
        Some(Price { input: 1.25, cache_write: 0.0, cache_read: 0.125, output: 10.0 })
    } else if m.contains("gemini") {
        Some(Price { input: 1.25, cache_write: 0.0, cache_read: 0.31, output: 10.0 })
    } else {
        None
    }
}

/// Returns (cost_usd, was_model_priced).
pub fn cost(model: &str, input: u64, cache_write: u64, cache_read: u64, output: u64) -> (f64, bool) {
    match price_for(model) {
        Some(p) => {
            let c = (input as f64 * p.input
                + cache_write as f64 * p.cache_write
                + cache_read as f64 * p.cache_read
                + output as f64 * p.output)
                / 1_000_000.0;
            (c, true)
        }
        None => (0.0, false),
    }
}
