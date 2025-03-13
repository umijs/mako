use tokio::time::Duration;
use tokio_retry::strategy::ExponentialBackoff;

pub fn create_retry_strategy() -> impl Iterator<Item = Duration> {
    let delays = vec![
        Duration::from_millis(100), // 100ms
        Duration::from_millis(200), // 200ms
        Duration::from_secs(1),     // 1s
        Duration::from_secs(1),     // 1s
        Duration::from_secs(1),     // 1s
    ];
    let exp_strategy = ExponentialBackoff::from_millis(1000)
        .max_delay(Duration::from_secs(20))
        .take(5); // 5 fixed delays

    delays.into_iter().chain(exp_strategy)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fixed_delays() {
        let strategy = create_retry_strategy();
        let delays: Vec<Duration> = strategy.take(5).collect();

        assert_eq!(delays[0], Duration::from_millis(100));
        assert_eq!(delays[1], Duration::from_millis(200));
        assert_eq!(delays[2], Duration::from_secs(1));
        assert_eq!(delays[3], Duration::from_secs(1));
        assert_eq!(delays[4], Duration::from_secs(1));
    }

    #[test]
    fn test_total_retry_count() {
        let strategy = create_retry_strategy();
        let delays: Vec<Duration> = strategy.collect();

        assert_eq!(delays.len(), 10);
    }

    #[test]
    fn test_max_delay_limit() {
        let strategy = create_retry_strategy();
        let max_delay = Duration::from_secs(20);

        for delay in strategy {
            assert!(delay <= max_delay, "Delay should not exceed 20 seconds");
        }
    }
}
