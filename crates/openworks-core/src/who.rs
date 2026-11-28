/// An opaque identifier the host hands out.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Id(pub String);

/// One of the five words an operation needs and a role grants.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Word {
    Read,
    Write,
    Approve,
    Members,
    Settings,
}

impl Word {
    /// Every word.
    pub const ALL: [Self; 5] = [Self::Read, Self::Write, Self::Approve, Self::Members, Self::Settings];

    /// The word as a caller sees it.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Read => "read",
            Self::Write => "write",
            Self::Approve => "approve",
            Self::Members => "members",
            Self::Settings => "settings",
        }
    }
}

/// How far a granted word reaches: every record in the org, or only the person's own.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Scope {
    All,
    Own,
}

/// A member's role in an org.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Role {
    Owner,
    Manager,
    Member,
    Viewer,
}

impl Role {
    /// Every role.
    pub const ALL: [Self; 4] = [Self::Owner, Self::Manager, Self::Member, Self::Viewer];

    /// The role as it is stored and read.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Owner => "owner",
            Self::Manager => "manager",
            Self::Member => "member",
            Self::Viewer => "viewer",
        }
    }

    /// The role with this name.
    pub fn named(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|role| role.name() == name)
    }

    /// The role table: how far this role holds the word, if at all.
    pub const fn grants(self, word: Word) -> Option<Scope> {
        match (self, word) {
            (Self::Owner, _)
            | (Self::Manager, Word::Read | Word::Write | Word::Approve)
            | (Self::Viewer, Word::Read) => Some(Scope::All),
            (Self::Member, Word::Read | Word::Write) => Some(Scope::Own),
            _ => None,
        }
    }
}

/// Who is at the keyboard: the person, or an assistant acting for them that may propose and never decide.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Via {
    Person,
    Assistant,
}

/// The signed-in member, the org they act for, their role in it and who acts for them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Who {
    pub person: Id,
    pub org: Id,
    pub role: Role,
    pub via: Via,
}
