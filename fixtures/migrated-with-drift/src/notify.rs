//! Status notifications, retried when the receiving side fails.
//!
//! A notification is attempted up to [`MAX_NOTIFY_ATTEMPTS`] times. The wait
//! before the second attempt is [`BASE_DELAY_MS`] milliseconds and each wait
//! after that is twice the one before it. The wait is a parameter so a test
//! does not have to sleep.

use std::fmt;
use std::time::Duration;

/// The most times one notification is attempted before it is given up.
pub const MAX_NOTIFY_ATTEMPTS: u32 = 5;

/// The wait before the second attempt, in milliseconds.
pub const BASE_DELAY_MS: u64 = 200;

/// Something a notification can be handed to.
pub trait Sender {
    /// Deliver `message`, or say why that did not work.
    fn send(&mut self, message: &str) -> Result<(), String>;
}

/// Every attempt failed.
#[derive(Debug, PartialEq, Eq)]
pub struct NotifyError {
    /// How many attempts were made.
    pub attempts: u32,
    /// What the last attempt reported.
    pub last: String,
}

impl fmt::Display for NotifyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "notification failed after {} attempts: {}",
            self.attempts, self.last
        )
    }
}

impl std::error::Error for NotifyError {}

/// Send `message`, retrying with a doubling wait, and return the number of
/// attempts it took. `wait` is called with each delay; pass
/// `std::thread::sleep` in production.
pub fn notify_with_retry<S: Sender, W: FnMut(Duration)>(
    sender: &mut S,
    message: &str,
    mut wait: W,
) -> Result<u32, NotifyError> {
    let mut delay = Duration::from_millis(BASE_DELAY_MS);
    let mut last = String::new();
    for attempt in 1..=MAX_NOTIFY_ATTEMPTS {
        match sender.send(message) {
            Ok(()) => return Ok(attempt),
            Err(reason) => last = reason,
        }
        if attempt < MAX_NOTIFY_ATTEMPTS {
            wait(delay);
            delay *= 2;
        }
    }
    Err(NotifyError {
        attempts: MAX_NOTIFY_ATTEMPTS,
        last,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Flaky {
        failures_left: u32,
        sent: Vec<String>,
    }

    impl Sender for Flaky {
        fn send(&mut self, message: &str) -> Result<(), String> {
            if self.failures_left > 0 {
                self.failures_left -= 1;
                return Err("receiver unavailable".to_owned());
            }
            self.sent.push(message.to_owned());
            Ok(())
        }
    }

    #[test]
    fn a_sender_that_recovers_is_retried_with_a_doubling_wait() {
        let mut sender = Flaky {
            failures_left: 3,
            sent: Vec::new(),
        };
        let mut waits = Vec::new();
        let attempts = notify_with_retry(&mut sender, "PK-1 delivered", |d| waits.push(d))
            .expect("fourth attempt succeeds");
        assert_eq!(attempts, 4);
        assert_eq!(sender.sent, vec!["PK-1 delivered".to_owned()]);
        assert_eq!(
            waits,
            vec![
                Duration::from_millis(200),
                Duration::from_millis(400),
                Duration::from_millis(800)
            ]
        );
    }

    #[test]
    fn a_sender_that_never_recovers_is_given_up_on() {
        let mut sender = Flaky {
            failures_left: u32::MAX,
            sent: Vec::new(),
        };
        let error = notify_with_retry(&mut sender, "PK-1 delivered", |_| {})
            .expect_err("every attempt fails");
        assert_eq!(error.attempts, MAX_NOTIFY_ATTEMPTS);
        assert!(sender.sent.is_empty());
    }
}
