use super::*;

fn in_jitter(base: Duration, wait: Duration) -> bool {
    let lo = base.as_secs_f64() * 0.8;
    let hi = base.as_secs_f64() * 1.2;
    let v = wait.as_secs_f64();
    (lo..=hi).contains(&v)
}

#[test]
fn step_doubles_with_jitter_and_caps() {
    let mut b = ReconnectBackoff::new();
    let mut base = Duration::from_millis(500);
    for _ in 0..10 {
        let wait = b.step();
        assert!(in_jitter(base, wait), "{wait:?} vs base {base:?}");
        base = (base * 2).min(Duration::from_secs(30));
    }
}

#[test]
fn reset_restores_initial_after_cap() {
    let mut b = ReconnectBackoff::new();
    for _ in 0..12 {
        b.step();
    }
    b.reset();
    let wait = b.step();
    assert!(in_jitter(Duration::from_millis(500), wait), "{wait:?}");
}

#[test]
fn jitter_actually_spreads_samples() {
    let mut samples = std::collections::HashSet::new();
    for _ in 0..64 {
        let mut b = ReconnectBackoff::new();
        samples.insert(b.step().as_millis());
    }
    assert!(samples.len() > 4, "jitter collapsed: {samples:?}");
}
