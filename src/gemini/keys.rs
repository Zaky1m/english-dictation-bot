use std::time::{Duration, Instant};

const DEFAULT_COOLDOWN: Duration = Duration::from_secs(60);
const AUTH_DISABLE: Duration = Duration::from_secs(24 * 60 * 60);

/// Round-robin pool of API keys. Pure logic: callers pass `now`, so tests can
/// drive time without sleeping.
pub struct KeyPool {
    keys: Vec<Slot>,
    next: usize,
}

struct Slot {
    key: String,
    unavailable_until: Option<Instant>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Pick {
    Key {
        index: usize,
    },
    /// Every key is cooling down; the earliest one frees up at this instant.
    WaitUntil(Instant),
    /// The pool has no keys at all.
    Empty,
}

impl KeyPool {
    pub fn new(keys: Vec<String>) -> Self {
        let keys = keys
            .into_iter()
            .map(|key| Slot {
                key,
                unavailable_until: None,
            })
            .collect();
        Self { keys, next: 0 }
    }

    pub fn key(&self, index: usize) -> &str {
        &self.keys[index].key
    }

    pub fn pick(&mut self, now: Instant) -> Pick {
        let total = self.keys.len();
        if total == 0 {
            return Pick::Empty;
        }
        for offset in 0..total {
            let index = (self.next + offset) % total;
            let free = self.keys[index].unavailable_until.is_none_or(|t| t <= now);
            if free {
                self.next = (index + 1) % total;
                return Pick::Key { index };
            }
        }
        let earliest = self
            .keys
            .iter()
            .filter_map(|slot| slot.unavailable_until)
            .min()
            .unwrap_or(now);
        Pick::WaitUntil(earliest)
    }

    pub fn rate_limited(&mut self, index: usize, now: Instant, retry_after: Option<Duration>) {
        self.keys[index].unavailable_until = Some(now + retry_after.unwrap_or(DEFAULT_COOLDOWN));
    }

    pub fn auth_failed(&mut self, index: usize, now: Instant) {
        self.keys[index].unavailable_until = Some(now + AUTH_DISABLE);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pool(n: usize) -> KeyPool {
        KeyPool::new((0..n).map(|i| format!("k{i}")).collect())
    }

    #[test]
    fn rotates_round_robin() {
        let mut p = pool(3);
        let now = Instant::now();
        let picks: Vec<_> = (0..5).map(|_| p.pick(now)).collect();
        let idx: Vec<_> = picks
            .into_iter()
            .map(|p| match p {
                Pick::Key { index } => index,
                other => panic!("unexpected {other:?}"),
            })
            .collect();
        assert_eq!(idx, [0, 1, 2, 0, 1]);
    }

    #[test]
    fn rate_limited_key_is_skipped_until_cooldown_ends() {
        let mut p = pool(2);
        let t0 = Instant::now();
        p.rate_limited(0, t0, Some(Duration::from_secs(10)));
        assert_eq!(p.pick(t0), Pick::Key { index: 1 });
        assert_eq!(p.pick(t0), Pick::Key { index: 1 });
        assert_eq!(p.pick(t0 + Duration::from_secs(10)), Pick::Key { index: 0 });
    }

    #[test]
    fn default_cooldown_is_sixty_seconds() {
        let mut p = pool(1);
        let t0 = Instant::now();
        p.rate_limited(0, t0, None);
        assert_eq!(p.pick(t0), Pick::WaitUntil(t0 + Duration::from_secs(60)));
        assert_eq!(p.pick(t0 + Duration::from_secs(60)), Pick::Key { index: 0 });
    }

    #[test]
    fn all_cooling_down_reports_earliest_release() {
        let mut p = pool(3);
        let t0 = Instant::now();
        p.rate_limited(0, t0, Some(Duration::from_secs(30)));
        p.rate_limited(1, t0, Some(Duration::from_secs(5)));
        p.rate_limited(2, t0, Some(Duration::from_secs(20)));
        assert_eq!(p.pick(t0), Pick::WaitUntil(t0 + Duration::from_secs(5)));
    }

    #[test]
    fn auth_failure_disables_for_a_day() {
        let mut p = pool(2);
        let t0 = Instant::now();
        p.auth_failed(0, t0);
        assert_eq!(
            p.pick(t0 + Duration::from_secs(3600)),
            Pick::Key { index: 1 }
        );
        assert_eq!(
            p.pick(t0 + Duration::from_secs(3600)),
            Pick::Key { index: 1 }
        );
        assert_eq!(
            p.pick(t0 + Duration::from_secs(24 * 3600)),
            Pick::Key { index: 0 }
        );
    }

    #[test]
    fn empty_pool_is_reported() {
        assert_eq!(pool(0).pick(Instant::now()), Pick::Empty);
    }
}
