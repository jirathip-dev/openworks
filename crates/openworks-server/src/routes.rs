use openworks::{Caller, Via};

use crate::http::Answer;

/// Who may call one of the server's own routes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gate {
    Anyone,
    Signed,
    Person,
}

impl Gate {
    /// The refusal this gate gives `caller`, if it does not let them through.
    pub fn refuses(self, caller: Option<&Caller>) -> Option<Answer> {
        match (self, caller) {
            (Self::Signed | Self::Person, None) => Some(Answer::text(401, "sign in")),
            (Self::Person, Some(caller)) if caller.via != Via::Person => {
                Some(Answer::text(403, "only the person may do this, not their assistant"))
            }
            (Self::Anyone, _) | (Self::Signed | Self::Person, Some(_)) => None,
        }
    }
}

/// A route the server answers itself, beside the core's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Own {
    Config,
    SignIn,
    Tokens,
    SignOut,
    OpenApi,
    Inbox,
    Close,
}

impl Own {
    /// Every route the server answers itself.
    pub const ALL: [Self; 7] = [
        Self::Config,
        Self::SignIn,
        Self::Tokens,
        Self::SignOut,
        Self::OpenApi,
        Self::Inbox,
        Self::Close,
    ];

    /// The method and path; a path ending in `/` takes any rest.
    pub const fn route(self) -> (&'static str, &'static str) {
        match self {
            Self::Config => ("GET", "/auth/config"),
            Self::SignIn => ("POST", "/auth/sign-in"),
            Self::Tokens => ("POST", "/auth/tokens"),
            Self::SignOut => ("POST", "/auth/sign-out"),
            Self::OpenApi => ("GET", "/openapi.json"),
            Self::Inbox => ("GET", "/inbox"),
            Self::Close => ("POST", "/inbox/"),
        }
    }

    /// Who may call it.
    pub const fn gate(self) -> Gate {
        match self {
            Self::Config | Self::SignIn | Self::OpenApi => Gate::Anyone,
            Self::SignOut => Gate::Signed,
            Self::Tokens | Self::Inbox | Self::Close => Gate::Person,
        }
    }

    /// The route a request names, if the server answers it itself.
    pub fn find(method: &str, path: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|own| {
            let (own_method, own_path) = own.route();
            own_method == method && (own_path == path || (own_path.ends_with('/') && path.starts_with(own_path)))
        })
    }
}
