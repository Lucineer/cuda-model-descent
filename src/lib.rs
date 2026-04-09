//! Model Descent — the ultimate decomposition system
//! Start with expensive model + thin agent → over time, cheaper model suffices
//! because algorithms absorb what inference used to do.

use std::collections::HashMap;

/// A model tier in the descent curve
#[derive(Debug, Clone)]
pub struct ModelTier {
    pub name: String,
    pub cost_per_1k_tokens: f64, // in dollars
    pub capability_level: f64,    // 0.0-1.0
    pub max_context_tokens: usize,
    pub avg_latency_ms: u64,
}

/// Known model tiers
pub fn known_tiers() -> Vec<ModelTier> {
    vec![
        ModelTier { name: "opus-4.6".to_string(), cost_per_1k_tokens: 0.075, capability_level: 1.0, max_context_tokens: 200000, avg_latency_ms: 3000 },
        ModelTier { name: "sonnet-4.6".to_string(), cost_per_1k_tokens: 0.015, capability_level: 0.85, max_context_tokens: 200000, avg_latency_ms: 1500 },
        ModelTier { name: "haiku-4.6".to_string(), cost_per_1k_tokens: 0.002, capability_level: 0.5, max_context_tokens: 200000, avg_latency_ms: 500 },
        ModelTier { name: "algorithmic".to_string(), cost_per_1k_tokens: 0.0001, capability_level: 0.3, max_context_tokens: 0, avg_latency_ms: 1 },
        ModelTier { name: "cached".to_string(), cost_per_1k_tokens: 0.0, capability_level: 0.2, max_context_tokens: 0, avg_latency_ms: 0 },
    ]
}

/// A prompt classification — what kind of processing does this request need?
#[derive(Debug, Clone, PartialEq)]
pub enum PromptClass {
    /// Can be solved purely algorithmically (no inference needed)
    Algorithmic,
    /// Needs simulation/RNG on a graph or spreadsheet logic
    Simulation,
    /// Needs black-box model inference
    Inference,
    /// Hybrid — some parts algorithmic, some inferenced
    Hybrid { algorithmic_pct: f64, simulation_pct: f64, inference_pct: f64 },
}

/// A prompt router — classifies prompts and routes to cheapest adequate tier
pub struct PromptRouter {
    algorithm_cache: HashMap<String, PromptClass>,
    total_prompts: u64,
    algorithmic_resolved: u64,
    inference_required: u64,
}

impl PromptRouter {
    pub fn new() -> Self {
        Self { algorithm_cache: HashMap::new(), total_prompts: 0, algorithmic_resolved: 0, inference_required: 0 }
    }

    /// Classify a prompt — determine what processing it needs
    pub fn classify(&mut self, prompt: &str) -> PromptClass {
        self.total_prompts += 1;
        let lower = prompt.to_lowercase();

        // Check cache for similar prompts
        let cache_key = normalize_prompt(&lower);
        if let Some(cached) = self.algorithm_cache.get(&cache_key) {
            return cached.clone();
        }

        // Classification heuristics
        let is_sort = lower.contains("sort") || lower.contains("order");
        let is_filter = lower.contains("filter") || lower.contains("only");
        let is_math = lower.contains("calculate") || lower.contains("compute");
        let is_creative = lower.contains("write") || lower.contains("create") || lower.contains("design");
        let is_complex = prompt.split_whitespace().count() > 20;

        let classification = if is_creative && is_complex {
            PromptClass::Inference
        } else if is_sort || is_filter || is_math {
            PromptClass::Algorithmic
        } else if is_complex {
            PromptClass::Hybrid { algorithmic_pct: 0.4, simulation_pct: 0.2, inference_pct: 0.4 }
        } else {
            PromptClass::Hybrid { algorithmic_pct: 0.5, simulation_pct: 0.3, inference_pct: 0.2 }
        };

        self.algorithm_cache.insert(cache_key, classification.clone());
        if matches!(classification, PromptClass::Algorithmic) {
            self.algorithmic_resolved += 1;
        } else {
            self.inference_required += 1;
        }
        classification
    }

    /// Route to the cheapest model tier that can handle this classification
    pub fn route(&self, classification: &PromptClass, min_capability: f64) -> &str {
        let required_cap = match classification {
            PromptClass::Algorithmic => 0.0,
            PromptClass::Simulation => 0.3,
            PromptClass::Inference => min_capability,
            PromptClass::Hybrid { inference_pct, .. } => min_capability * inference_pct,
        };
        let tiers = known_tiers();
        // Find cheapest tier with enough capability
        for tier in tiers.iter().rev() {
            if tier.capability_level >= required_cap {
                return &tier.name;
            }
        }
        &tiers[0].name // fallback to most capable
    }

    /// Get algorithm absorption rate
    pub fn absorption_rate(&self) -> f64 {
        if self.total_prompts == 0 { return 0.0; }
        self.algorithmic_resolved as f64 / self.total_prompts as f64
    }

    /// Estimate cost savings from algorithm absorption
    pub fn cost_savings(&self, base_cost: f64) -> f64 {
        base_cost * self.absorption_rate()
    }
}

/// Intelligence absorption tracker — monitors how much inference is replaced by algorithms
pub struct AbsorptionTracker {
    history: Vec<AbsorptionPoint>,
}

#[derive(Debug, Clone)]
pub struct AbsorptionPoint {
    pub session_id: usize,
    pub total_requests: u64,
    pub inference_requests: u64,
    pub algorithm_requests: u64,
    pub absorption_rate: f64,
    pub cost_savings_pct: f64,
}

impl AbsorptionTracker {
    pub fn new() -> Self { Self { history: vec![] } }

    pub fn record(&mut self, point: AbsorptionPoint) {
        self.history.push(point);
    }

    /// Predict when full absorption occurs (no inference needed)
    pub fn predict_full_absorption(&self) -> Option<usize> {
        if self.history.len() < 3 { return None; }
        let recent: Vec<f64> = self.history.iter().rev().take(5).map(|h| h.absorption_rate).collect();
        if recent.len() < 2 { return None; }
        let rate_of_change = recent[0] - recent[recent.len().min(5) - 1];
        if rate_of_change <= 0.0 { return None; }
        let remaining = 1.0 - recent[0];
        let sessions_needed = (remaining / rate_of_change).ceil() as usize;
        let current_session = self.history.last()?.session_id;
        Some(current_session + sessions_needed)
    }

    pub fn current_absorption(&self) -> f64 {
        self.history.last().map(|h| h.absorption_rate).unwrap_or(0.0)
    }
}

fn normalize_prompt(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_sort() {
        let mut router = PromptRouter::new();
        assert_eq!(router.classify("sort a list of numbers"), PromptClass::Algorithmic);
    }

    #[test]
    fn test_classify_creative() {
        let mut router = PromptRouter::new();
        let result = router.classify("write a creative story about fishing in Alaska with detailed character development and plot twists");
        assert_eq!(result, PromptClass::Inference);
    }

    #[test]
    fn test_route_algorithmic() {
        let router = PromptRouter::new();
        assert_eq!(router.route(&PromptClass::Algorithmic, 0.5), "algorithmic");
    }

    #[test]
    fn test_route_inference() {
        let router = PromptRouter::new();
        assert_eq!(router.route(&PromptClass::Inference, 0.9), "opus-4.6");
    }

    #[test]
    fn test_absorption_prediction() {
        let mut tracker = AbsorptionTracker::new();
        for i in 1..=10 {
            tracker.record(AbsorptionPoint {
                session_id: i, total_requests: 100,
                inference_requests: (100 - i * 8).max(0) as u64,
                algorithm_requests: (i * 8).min(100) as u64,
                absorption_rate: (i * 8) as f64 / 100.0,
                cost_savings_pct: (i * 8) as f64 / 100.0,
            });
        }
        let pred = tracker.predict_full_absorption();
        assert!(pred.is_some());
        assert!(pred.unwrap() < 20);
    }

    #[test]
    fn test_known_tiers() {
        let tiers = known_tiers();
        assert_eq!(tiers.len(), 5);
        assert!(tiers[0].cost_per_1k_tokens > tiers[1].cost_per_1k_tokens);
    }
}
