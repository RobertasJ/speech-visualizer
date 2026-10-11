//! What the agent's requests used, in tokens, and what that cost in dollars.

use std::ops::AddAssign;

/// Tokens used, summed over requests, and their price in dollars.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct Usage {
    pub requests: u32,
    /// Input tokens not read from or written to the cache.
    pub input: u64,
    pub cache_read: u64,
    pub cache_write: u64,
    /// Output tokens, thinking included.
    pub output: u64,
    /// The part of `output` spent thinking.
    pub thinking: u64,
    /// None if the model's prices aren't known.
    pub dollars: Option<f64>,
}

impl AddAssign for Usage {
    fn add_assign(&mut self, other: Usage) {
        // Unknown if the price of any request in it is.
        self.dollars = if self.requests == 0 {
            other.dollars
        } else if other.requests == 0 {
            self.dollars
        } else {
            self.dollars.zip(other.dollars).map(|(a, b)| a + b)
        };
        self.requests += other.requests;
        self.input += other.input;
        self.cache_read += other.cache_read;
        self.cache_write += other.cache_write;
        self.output += other.output;
        self.thinking += other.thinking;
    }
}

/// Dollars per million tokens.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Prices {
    input: f64,
    cache_write: f64,
    cache_read: f64,
    output: f64,
}

/// A model's prices: `short` for prompts up to `long_from` tokens, `long` above that.
struct Model {
    id: &'static str,
    short: Prices,
    long: Option<(u64, Prices)>,
}

/// Anthropic's list prices, from platform.claude.com/docs/en/about-claude/pricing
/// (October 2026), with 5-minute cache writes.
const MODELS: [Model; 3] = [
    Model {
        id: "claude-haiku-5-5",
        short: Prices {
            input: 0.10,
            cache_write: 0.125,
            cache_read: 0.01,
            output: 0.50,
        },
        long: Some((
            100_000,
            Prices {
                input: 0.50,
                cache_write: 0.625,
                cache_read: 0.05,
                output: 2.50,
            },
        )),
    },
    Model {
        id: "claude-sonnet-5-5",
        short: Prices {
            input: 2.,
            cache_write: 2.50,
            cache_read: 0.10,
            output: 10.,
        },
        long: None,
    },
    Model {
        id: "claude-opus-5-5",
        short: Prices {
            input: 4.,
            cache_write: 5.,
            cache_read: 0.20,
            output: 20.,
        },
        long: None,
    },
];

/// What one request to `model` used, as rig reports it, and its price. Each request is
/// priced on its own, by its whole prompt's length.
pub fn request(model: &str, usage: &rig::completion::Usage) -> Usage {
    let prompt = usage.input_tokens.unwrap_or(0);
    let cache_read = usage.cached_input_tokens.unwrap_or(0);
    let cache_write = usage.cache_creation_input_tokens.unwrap_or(0);
    let mut used = Usage {
        requests: 1,
        input: prompt.saturating_sub(cache_read + cache_write),
        cache_read,
        cache_write,
        output: usage.output_tokens.unwrap_or(0),
        thinking: usage.reasoning_tokens.unwrap_or(0),
        dollars: None,
    };
    used.dollars = prices(model, prompt).map(|prices| dollars(&used, &prices));
    used
}

fn prices(model: &str, prompt: u64) -> Option<Prices> {
    let model = MODELS.iter().find(|known| known.id == model)?;
    Some(match model.long {
        Some((long_from, long)) if prompt > long_from => long,
        _ => model.short,
    })
}

fn dollars(used: &Usage, prices: &Prices) -> f64 {
    let millions = |tokens: u64| tokens as f64 / 1_000_000.;
    millions(used.input) * prices.input
        + millions(used.cache_write) * prices.cache_write
        + millions(used.cache_read) * prices.cache_read
        + millions(used.output) * prices.output
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reported(input: u64, read: u64, write: u64, output: u64) -> rig::completion::Usage {
        rig::completion::Usage::new()
            .input_tokens(input)
            .cached_input_tokens(read)
            .cache_creation_input_tokens(write)
            .output_tokens(output)
    }

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-12
    }

    #[test]
    fn prices_each_kind_of_token() {
        // 1M uncached input, 2M cache reads, 1M cache writes, 1M output.
        let used = request(
            "claude-sonnet-5-5",
            &reported(4_000_000, 2_000_000, 1_000_000, 1_000_000),
        );
        assert_eq!(
            (used.input, used.cache_read, used.cache_write),
            (1_000_000, 2_000_000, 1_000_000)
        );
        assert!(close(used.dollars.unwrap(), 2. + 0.2 + 2.5 + 10.));
    }

    #[test]
    fn haiku_costs_more_over_100k_prompt_tokens() {
        let short = request("claude-haiku-5-5", &reported(100_000, 0, 0, 1_000));
        assert!(close(short.dollars.unwrap(), 0.1 * 0.1 + 0.001 * 0.5));
        // Cache reads count towards the prompt's length.
        let long = request("claude-haiku-5-5", &reported(100_001, 50_000, 0, 1_000));
        assert!(close(
            long.dollars.unwrap(),
            0.050_001 * 0.5 + 0.05 * 0.05 + 0.001 * 2.5
        ));
    }

    #[test]
    fn unknown_models_have_no_price() {
        assert_eq!(request("some-model", &reported(10, 0, 0, 10)).dollars, None);
    }

    #[test]
    fn usage_adds_up() {
        let mut total = Usage::default();
        total += request("claude-opus-5-5", &reported(1_000_000, 0, 0, 0));
        total += request("claude-opus-5-5", &reported(0, 0, 0, 1_000_000));
        assert_eq!(
            (total.requests, total.input, total.output),
            (2, 1_000_000, 1_000_000)
        );
        assert!(close(total.dollars.unwrap(), 24.));
        // One unknown price makes the sum unknown.
        total += request("some-model", &reported(1, 0, 0, 1));
        assert_eq!(total.dollars, None);
    }
}
