use std::fmt;

/// Where a job is in its life; the job rules read only this.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum JobStatus {
    Proposed,
    Open,
    Finished,
    Cancelled,
}

impl JobStatus {
    /// Every status.
    pub const ALL: [Self; 4] = [Self::Proposed, Self::Open, Self::Finished, Self::Cancelled];

    /// The status as it is stored and read.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Proposed => "proposed",
            Self::Open => "open",
            Self::Finished => "finished",
            Self::Cancelled => "cancelled",
        }
    }

    /// The status with this name.
    pub fn named(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|status| status.name() == name)
    }

    /// Where a step takes a job in this status, as the job statechart draws it; `None` refuses the step.
    pub const fn after(self, step: JobStep) -> Option<Self> {
        match (self, step) {
            (Self::Proposed, JobStep::Reschedule) | (Self::Open, JobStep::Unwin) => Some(Self::Proposed),
            (Self::Proposed, JobStep::Open)
            | (Self::Open, JobStep::Reschedule | JobStep::Record)
            | (Self::Finished, JobStep::Reopen) => Some(Self::Open),
            (Self::Open, JobStep::Finish) => Some(Self::Finished),
            (Self::Open, JobStep::Cancel) => Some(Self::Cancelled),
            _ => None,
        }
    }
}

/// What can happen to a job.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum JobStep {
    Reschedule,
    Record,
    Open,
    Unwin,
    Finish,
    Reopen,
    Cancel,
}

impl JobStep {
    /// The step as a refusal names it.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Reschedule => "reschedule",
            Self::Record => "record actual dates",
            Self::Open => "open",
            Self::Unwin => "go back to proposed",
            Self::Finish => "finish",
            Self::Reopen => "reopen",
            Self::Cancel => "cancel",
        }
    }
}

/// A calendar day, read and written as `YYYY-MM-DD`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Date {
    year: u16,
    month: u8,
    day: u8,
}

impl Date {
    /// The day a `YYYY-MM-DD` text names, if it is one.
    pub fn parse(text: &str) -> Option<Self> {
        let [year, month, day] = text.split('-').collect::<Vec<_>>()[..] else {
            return None;
        };
        if (year.len(), month.len(), day.len()) != (4, 2, 2) || !text.bytes().all(|b| b == b'-' || b.is_ascii_digit()) {
            return None;
        }
        let (year, month, day): (u16, u8, u8) = (year.parse().ok()?, month.parse().ok()?, day.parse().ok()?);
        let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
        let last = match month {
            2 if leap => 29,
            2 => 28,
            4 | 6 | 9 | 11 => 30,
            1..=12 => 31,
            _ => return None,
        };
        (1..=last).contains(&day).then_some(Self { year, month, day })
    }
}

impl fmt::Display for Date {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        std::write!(f, "{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }
}

/// A range of days that cannot end before it starts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Window {
    starts_on: Date,
    ends_on: Date,
}

impl Window {
    /// The window from `starts_on` to `ends_on`, unless it ends first.
    pub fn new(starts_on: Date, ends_on: Date) -> Option<Self> {
        (starts_on <= ends_on).then_some(Self { starts_on, ends_on })
    }

    /// The first day.
    pub const fn starts_on(self) -> Date {
        self.starts_on
    }

    /// The last day.
    pub const fn ends_on(self) -> Date {
        self.ends_on
    }
}

/// The ISO weekdays, 1 for Monday to 7 for Sunday, that count as working days; at least one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct WorkDays(u8);

impl WorkDays {
    /// The weekdays a comma-separated list of ISO weekday numbers names, if it names at least one and no other number.
    pub fn parse(text: &str) -> Option<Self> {
        let mut days = 0_u8;
        for day in text.split(',').map(str::trim) {
            let day: u8 = day.parse().ok().filter(|day| (1..=7).contains(day))?;
            days |= 1 << day;
        }
        Some(Self(days))
    }

    /// Whether the ISO weekday counts.
    pub const fn counts(self, weekday: u8) -> bool {
        weekday >= 1 && weekday <= 7 && self.0 & (1 << weekday) != 0
    }
}

impl fmt::Display for WorkDays {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let days: Vec<String> = (1..=7)
            .filter(|day| self.counts(*day))
            .map(|day| day.to_string())
            .collect();
        f.write_str(&days.join(","))
    }
}
