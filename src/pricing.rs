/// Per-million-token standard API prices (USD), reviewed 2026-08-20.
///
/// Sources:
/// - https://platform.claude.com/docs/en/about-claude/pricing
/// - https://platform.openai.com/pricing
///
/// An editable price table will replace this later.
pub struct Price {
    pub input: f64,
    pub cache_write: f64,
    pub cache_read: f64,
    pub output: f64,
}

pub fn price_for(model: &str) -> Option<Price> {
    let m = model.to_lowercase();

    // Opus 4 and 4.1 retained the older $15/$75 pricing. Newer Opus 4.x
    // models use the lower $5/$25 rate, so a broad `contains("opus")`
    // check silently underprices legacy sessions by 3x.
    if m.contains("opus-4-1") || m.contains("opus-4-2025") || m.contains("claude-3-opus") {
        Some(Price {
            input: 15.0,
            cache_write: 18.75,
            cache_read: 1.5,
            output: 75.0,
        })
    } else if m.contains("opus-4-5")
        || m.contains("opus-4-6")
        || m.contains("opus-4-7")
        || m.contains("opus-4-8")
    {
        Some(Price {
            input: 5.0,
            cache_write: 6.25,
            cache_read: 0.5,
            output: 25.0,
        })
    } else if m.contains("sonnet-4")
        || m.contains("sonnet-3")
        || m.contains("3-7-sonnet")
        || m.contains("3-5-sonnet")
        || m.contains("claude-3-sonnet")
    {
        Some(Price {
            input: 3.0,
            cache_write: 3.75,
            cache_read: 0.3,
            output: 15.0,
        })
    } else if m.contains("haiku-4-5") {
        Some(Price {
            input: 1.0,
            cache_write: 1.25,
            cache_read: 0.1,
            output: 5.0,
        })
    } else if m.contains("haiku-3-5") || m.contains("claude-3-5-haiku") {
        Some(Price {
            input: 0.8,
            cache_write: 1.0,
            cache_read: 0.08,
            output: 4.0,
        })
    } else if m.starts_with("gpt-5.6-luna") {
        Some(Price {
            input: 0.2,
            cache_write: 0.25,
            cache_read: 0.02,
            output: 1.2,
        })
    } else if m.starts_with("gpt-5.6-terra") {
        Some(Price {
            input: 2.0,
            cache_write: 2.5,
            cache_read: 0.2,
            output: 12.0,
        })
    } else if m == "gpt-5.6" || m.starts_with("gpt-5.6-sol") {
        Some(Price {
            input: 5.0,
            cache_write: 6.25,
            cache_read: 0.5,
            output: 30.0,
        })
    } else if m.starts_with("gpt-5.5-pro") {
        Some(Price {
            input: 30.0,
            cache_write: 0.0,
            cache_read: 30.0,
            output: 180.0,
        })
    } else if m.starts_with("gpt-5.5") {
        Some(Price {
            input: 5.0,
            cache_write: 0.0,
            cache_read: 0.5,
            output: 30.0,
        })
    } else if m.starts_with("gpt-5.4-mini") {
        Some(Price {
            input: 0.75,
            cache_write: 0.0,
            cache_read: 0.075,
            output: 4.5,
        })
    } else if m.starts_with("gpt-5.4-nano") {
        Some(Price {
            input: 0.2,
            cache_write: 0.0,
            cache_read: 0.02,
            output: 1.25,
        })
    } else if m.starts_with("gpt-5.4") {
        Some(Price {
            input: 2.5,
            cache_write: 0.0,
            cache_read: 0.25,
            output: 15.0,
        })
    } else if m.starts_with("gpt-5.3-codex") || m.starts_with("gpt-5.2") {
        Some(Price {
            input: 1.75,
            cache_write: 0.0,
            cache_read: 0.175,
            output: 14.0,
        })
    } else if m.starts_with("gpt-5.1-codex-mini") {
        Some(Price {
            input: 0.25,
            cache_write: 0.0,
            cache_read: 0.025,
            output: 2.0,
        })
    } else if m.starts_with("gpt-5.1") || m.starts_with("gpt-5-codex") || m == "gpt-5" {
        Some(Price {
            input: 1.25,
            cache_write: 0.0,
            cache_read: 0.125,
            output: 10.0,
        })
    } else if m.contains("gemini") {
        Some(Price {
            input: 1.25,
            cache_write: 0.0,
            cache_read: 0.31,
            output: 10.0,
        })
    } else {
        None
    }
}

/// Returns (cost_usd, was_model_priced).
pub fn cost(
    model: &str,
    input: u64,
    cache_write: u64,
    cache_read: u64,
    output: u64,
) -> (f64, bool) {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_supported_model_families() {
        assert!(price_for("claude-opus-4-1").is_some());
        assert!(price_for("claude-sonnet-4").is_some());
        assert!(price_for("gpt-5-codex").is_some());
        assert!(price_for("gpt-5.6-terra").is_some());
        assert!(price_for("gpt-5.4-mini").is_some());
        assert!(price_for("gpt-4o").is_none());
        assert!(price_for("unknown-local-model").is_none());
    }

    #[test]
    fn distinguishes_legacy_and_current_opus_prices() {
        let legacy = price_for("claude-opus-4-1").unwrap();
        let current = price_for("claude-opus-4-8").unwrap();

        assert_eq!(legacy.input, 15.0);
        assert_eq!(legacy.output, 75.0);
        assert_eq!(current.input, 5.0);
        assert_eq!(current.output, 25.0);
    }

    #[test]
    fn distinguishes_current_openai_model_tiers() {
        let sol = price_for("gpt-5.6-sol").unwrap();
        let terra = price_for("gpt-5.6-terra").unwrap();
        let luna = price_for("gpt-5.6-luna").unwrap();

        assert_eq!((sol.input, sol.cache_read, sol.output), (5.0, 0.5, 30.0));
        assert_eq!(
            (terra.input, terra.cache_read, terra.output),
            (2.0, 0.2, 12.0)
        );
        assert_eq!((luna.input, luna.cache_read, luna.output), (0.2, 0.02, 1.2));
    }

    #[test]
    fn computes_per_million_token_cost() {
        let (cost, priced) = cost("gpt-5-codex", 1_000_000, 0, 1_000_000, 1_000_000);
        assert!(priced);
        assert!((cost - 11.375).abs() < f64::EPSILON);
    }
}
