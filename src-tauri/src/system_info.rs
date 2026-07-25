use sysinfo::System;

/// `System::total_memory()` returns bytes (since sysinfo 0.26.0).
pub fn total_ram_gb() -> f32 {
    let mut sys = System::new_all();
    sys.refresh_memory();
    sys.total_memory() as f32 / 1024.0 / 1024.0 / 1024.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn total_ram_gb_returns_positive_value() {
        let ram = total_ram_gb();
        assert!(ram > 0.0, "expected total_ram_gb() > 0, got {ram}");
    }
}
