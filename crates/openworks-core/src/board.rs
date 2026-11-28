/// What a planner must settle on a job before its days, computed on each read and never stored.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Issue {
    NoTasks,
    OpenSeat,
    OpenNeed,
    Clash,
}

impl Issue {
    /// The issue as a read names it.
    pub const fn name(self) -> &'static str {
        match self {
            Self::NoTasks => "no_tasks",
            Self::OpenSeat => "open_seat",
            Self::OpenNeed => "open_need",
            Self::Clash => "clash",
        }
    }
}

/// What the board counts on one job over the days it reads.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Readiness {
    pub tasks: usize,
    pub open_seats: usize,
    pub open_needs: usize,
    pub clashes: usize,
}

impl Readiness {
    /// The job's issues in a fixed order; a job with none is ready.
    pub fn issues(self) -> Vec<Issue> {
        [
            (self.tasks == 0, Issue::NoTasks),
            (self.open_seats > 0, Issue::OpenSeat),
            (self.open_needs > 0, Issue::OpenNeed),
            (self.clashes > 0, Issue::Clash),
        ]
        .into_iter()
        .filter_map(|(holds, issue)| holds.then_some(issue))
        .collect()
    }
}
