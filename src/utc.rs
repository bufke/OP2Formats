//! Calendar dates of Unix times, in UTC, without a date crate: the stamps in op2-rs's recording names and listings and
//! op2-perf's report.

/// A Unix time (seconds since 1970-01-01 UTC) as its UTC date and time of day.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Utc {
    pub year: i64,
    pub month: u32,
    pub day: u32,
    pub hour: u32,
    pub minute: u32,
    pub second: u32,
}

impl Utc {
    pub fn of(secs: i64) -> Utc {
        let rem = secs.rem_euclid(86_400) as u32;
        // Civil date from days since 1970-01-01 (Howard Hinnant's algorithm).
        let z = secs.div_euclid(86_400) + 719_468;
        let era = z.div_euclid(146_097);
        let doe = z - era * 146_097;
        let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
        let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
        let year = yoe + era * 400 + i64::from(month <= 2);
        Utc { year, month, day, hour: rem / 3600, minute: rem % 3600 / 60, second: rem % 60 }
    }

    /// `YYYY-MM-DD`.
    pub fn date(&self) -> String {
        format!("{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }
}

#[cfg(test)]
mod tests {
    use super::Utc;

    #[test]
    fn known_dates() {
        assert_eq!(Utc::of(0), Utc { year: 1970, month: 1, day: 1, hour: 0, minute: 0, second: 0 });
        // 2000-02-29 23:59:59, a leap day in a century year divisible by 400.
        assert_eq!(Utc::of(951_868_799), Utc { year: 2000, month: 2, day: 29, hour: 23, minute: 59, second: 59 });
        assert_eq!(Utc::of(1_793_664_000).date(), "2026-11-03");
        assert_eq!(Utc::of(-1).date(), "1969-12-31");
    }
}
