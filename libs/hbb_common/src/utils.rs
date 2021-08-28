

use chrono::prelude::*;

pub fn now() -> u64 {
    Utc::now().timestamp_millis() as u64
}