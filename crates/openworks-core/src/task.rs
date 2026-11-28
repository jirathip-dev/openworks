/// Hours in whole hundredths of an hour.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Hours(i64);

impl Hours {
    /// No hours.
    pub const ZERO: Self = Self(0);

    /// The hours a count of hundredths names, unless it is negative.
    pub const fn hundredths(count: i64) -> Option<Self> {
        if count < 0 { None } else { Some(Self(count)) }
    }

    /// The count of hundredths.
    pub const fn get(self) -> i64 {
        self.0
    }

    /// Whether a task with these hours reported on it may be archived: only one with none may.
    pub const fn lets_archive(self) -> bool {
        self.0 == 0
    }
}
