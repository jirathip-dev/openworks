use openworks_core::{Mount, Reach};

use crate::{
    approvals, clients, equipment, exports, files, job_orders, jobs, members, orgs, projects, setup, sites, tasks,
};

openworks_core::operations! {
    OrgList => orgs::LIST,
    OrgRename => orgs::RENAME,
    MemberList => members::LIST,
    MemberSet => members::SET,
    MemberRemove => members::REMOVE,
    FileAttach => files::ATTACH,
    FileList => files::LIST,
    ApprovalSaveKind => approvals::SAVE_KIND,
    ApprovalRequest => approvals::REQUEST,
    ApprovalDecide => approvals::DECIDE,
    ApprovalReset => approvals::RESET,
    ApprovalPending => approvals::PENDING,
    ApprovalRecent => approvals::RECENT,
    ClientList => clients::LIST,
    ClientGet => clients::GET,
    ClientSave => clients::SAVE,
    ClientDeactivate => clients::DEACTIVATE,
    ClientSync => clients::SYNC,
    SiteList => sites::LIST,
    SiteGet => sites::GET,
    SiteSave => sites::SAVE,
    SiteDeactivate => sites::DEACTIVATE,
    ProjectList => projects::LIST,
    ProjectGet => projects::GET,
    ProjectPage => projects::PAGE,
    ProjectChanges => projects::CHANGES,
    ProjectImport => crate::import::IMPORT,
    ProjectSave => projects::SAVE,
    ProjectMoveStage => projects::MOVE_STAGE,
    ProjectLink => projects::LINK,
    ProjectMembers => projects::MEMBERS,
    ProjectMembersSave => projects::MEMBERS_SAVE,
    ProjectArchive => projects::ARCHIVE,
    StageList => setup::STAGES,
    StageSave => setup::STAGE_SAVE,
    LostReasonList => setup::LOST_REASONS,
    LostReasonSave => setup::LOST_REASON_SAVE,
    ProjectRoleList => setup::ROLES,
    ProjectRoleSave => setup::ROLE_SAVE,
    JobList => jobs::LIST,
    JobBoard => jobs::BOARD,
    JobGet => jobs::GET,
    JobSave => jobs::SAVE,
    JobReschedule => jobs::RESCHEDULE,
    JobOpen => jobs::OPEN,
    JobFinish => jobs::FINISH,
    JobReopen => jobs::REOPEN,
    JobCancel => jobs::CANCEL,
    JobOrderList => job_orders::LIST,
    JobOrderGet => job_orders::GET,
    JobOrderPrint => job_orders::PRINT,
    JobOrderSave => job_orders::SAVE,
    JobOrderIssue => job_orders::ISSUE,
    TaskList => tasks::LIST,
    TaskByProject => tasks::BY_PROJECT,
    TaskSave => tasks::SAVE,
    TaskArchive => tasks::ARCHIVE,
    TaskRestore => tasks::RESTORE,
    EquipmentList => equipment::LIST,
    EquipmentSave => equipment::SAVE,
    EquipmentBook => equipment::BOOK_THING,
    EquipmentFill => equipment::FILL_NEED,
    EquipmentCancel => equipment::CANCEL,
    EquipmentNeed => equipment::NEED,
    EquipmentNeeds => equipment::NEEDS,
    EquipmentTimeline => equipment::TIMELINE,
    ExportCreate => exports::CREATE,
}

pub(crate) const KEY: &str = "orgs";

pub(crate) const CLIENTS: &str = "clients";

const HOMES: [(&str, &str); 11] = [
    ("/orgs", KEY),
    ("/files", "files"),
    ("/approvals", "approvals"),
    ("/clients", CLIENTS),
    ("/sites", "sites"),
    ("/projects", "projects"),
    ("/jobs", "jobs"),
    ("/job-orders", "job_orders"),
    ("/tasks", "tasks"),
    ("/equipment", "equipment"),
    ("/exports", "exports"),
];

impl Operation {
    /// The path prefix and extension key the operation is mounted under.
    pub const fn home(self) -> (&'static str, &'static str) {
        match self {
            Self::OrgList | Self::OrgRename | Self::MemberList | Self::MemberSet | Self::MemberRemove => ("/orgs", KEY),
            Self::FileAttach | Self::FileList => ("/files", "files"),
            Self::ApprovalSaveKind
            | Self::ApprovalRequest
            | Self::ApprovalDecide
            | Self::ApprovalReset
            | Self::ApprovalPending
            | Self::ApprovalRecent => ("/approvals", "approvals"),
            Self::ClientList | Self::ClientGet | Self::ClientSave | Self::ClientDeactivate | Self::ClientSync => {
                ("/clients", CLIENTS)
            }
            Self::SiteList | Self::SiteGet | Self::SiteSave | Self::SiteDeactivate => ("/sites", "sites"),
            Self::JobList
            | Self::JobBoard
            | Self::JobGet
            | Self::JobSave
            | Self::JobReschedule
            | Self::JobOpen
            | Self::JobFinish
            | Self::JobReopen
            | Self::JobCancel => ("/jobs", "jobs"),
            Self::JobOrderList | Self::JobOrderGet | Self::JobOrderPrint | Self::JobOrderSave | Self::JobOrderIssue => {
                ("/job-orders", "job_orders")
            }
            Self::TaskList | Self::TaskByProject | Self::TaskSave | Self::TaskArchive | Self::TaskRestore => {
                ("/tasks", "tasks")
            }
            Self::EquipmentList
            | Self::EquipmentSave
            | Self::EquipmentBook
            | Self::EquipmentFill
            | Self::EquipmentCancel
            | Self::EquipmentNeed
            | Self::EquipmentNeeds
            | Self::EquipmentTimeline => ("/equipment", "equipment"),
            Self::ExportCreate => ("/exports", "exports"),
            _ => ("/projects", "projects"),
        }
    }

    /// The operation's path.
    pub fn path(self) -> String {
        format!("{}/{}", self.home().0, self.declaration().name())
    }

    /// The core's mounts: every operation under its prefix, reached by the caller's org.
    pub fn mounts() -> Vec<Mount> {
        HOMES
            .iter()
            .map(|&(prefix, key)| {
                Self::ALL
                    .iter()
                    .filter(|operation| operation.home().0 == prefix)
                    .fold(Mount::new(prefix, key), |mount, operation| {
                        mount.operation(operation.declaration(), Reach::Org)
                    })
            })
            .collect()
    }
}
