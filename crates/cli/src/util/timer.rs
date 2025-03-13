use std::time::{SystemTime, UNIX_EPOCH};

pub struct Timer;

impl Timer {
    pub fn now_timestamp() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
    }

    pub fn now_timestamp_millis() -> u128 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis()
    }

    pub fn format_datetime() -> String {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();

        let secs = now.as_secs();
        let millis = now.subsec_millis();

        format!(
            "{:0>4}-{:0>2}-{:0>2} {:0>2}:{:0>2}:{:0>2}.{:0>3}",
            1970 + secs / 31536000,
            (secs % 31536000) / 2592000 + 1,
            (secs % 2592000) / 86400 + 1,
            (secs % 86400) / 3600,
            (secs % 3600) / 60,
            secs % 60,
            millis
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timestamp() {
        let ts = Timer::now_timestamp();
        assert!(ts > 0);
    }

    #[test]
    fn test_timestamp_millis() {
        let ts = Timer::now_timestamp_millis();
        assert!(ts > 0);
    }

    #[test]
    fn test_format_datetime() {
        let datetime = Timer::format_datetime();
        assert!(datetime.len() == 23); // YYYY-MM-DD HH:mm:ss.SSS
        assert!(datetime.contains("-"));
        assert!(datetime.contains(":"));
        assert!(datetime.contains("."));
    }
}
