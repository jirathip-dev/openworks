/// The kind of a company's stage; the stage rules read only this.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum StageKind {
    Open,
    Won,
    Done,
    Lost,
}

impl StageKind {
    /// Every kind.
    pub const ALL: [Self; 4] = [Self::Open, Self::Won, Self::Done, Self::Lost];

    /// The kind as it is stored and read.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Won => "won",
            Self::Done => "done",
            Self::Lost => "lost",
        }
    }

    /// The kind with this name.
    pub fn named(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.name() == name)
    }

    /// Whether a project at a stage of this kind may move to a stage of kind `to`.
    pub const fn moves_to(self, to: Self) -> bool {
        matches!(
            (self, to),
            (Self::Open, Self::Open | Self::Won | Self::Lost)
                | (Self::Lost, Self::Open)
                | (Self::Won, Self::Done | Self::Open)
                | (Self::Done, Self::Won)
        )
    }
}
