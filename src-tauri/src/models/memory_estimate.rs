#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Quant {
    Q4,
    Q5,
    Q8,
    F16,
}

impl Quant {
    fn bytes_per_weight(self) -> f32 {
        match self {
            Quant::Q4 => 0.5,
            Quant::Q5 => 0.625,
            Quant::Q8 => 1.0,
            Quant::F16 => 2.0,
        }
    }
}

/// Estimate: params × bytes_per_weight(quant) × 1.2 (1.2 = overhead margin
/// for KV cache/runtime, not a measured value — always labeled "estimate" in the UI).
pub fn estimate_ram_gb(params_billions: f32, quant: Quant) -> f32 {
    params_billions * quant.bytes_per_weight() * 1.2
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn q4_7b_model_is_in_expected_range() {
        let ram = estimate_ram_gb(7.0, Quant::Q4);
        assert!(
            (4.0..=5.0).contains(&ram),
            "expected Q4 7B estimate in 4-5GB, got {ram}"
        );
    }

    #[test]
    fn q8_is_larger_than_q4_for_same_model() {
        let q4 = estimate_ram_gb(7.0, Quant::Q4);
        let q8 = estimate_ram_gb(7.0, Quant::Q8);
        assert!(q8 > q4, "expected Q8 ({q8}) > Q4 ({q4})");
    }
}
