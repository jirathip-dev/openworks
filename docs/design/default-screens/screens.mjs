const links = [
  { id: "today", label: "Today", icon: "☀" },
  { id: "planner", label: "Planner", icon: "▤" },
  { id: "projects", label: "Projects and sites", icon: "◆" },
  { id: "orders", label: "Job orders", icon: "☰" },
  { id: "reports", label: "Daily reports", icon: "✎" },
  { id: "approvals", label: "Approvals", icon: "✓" },
  { id: "crew", label: "Crew and equipment", icon: "◉" },
];

const nav = (current, filled) => ({
  type: "sidebar",
  id: "nav",
  label: "openworks",
  brand: { label: filled ? "Northgate Works" : "New organisation", href: "#today", icon: "◈" },
  groups: [
    {
      links: links.map((l) => ({
        label: l.label,
        href: `#${l.id}`,
        icon: l.icon,
        ...(l.id === current ? { current: true } : {}),
        ...(filled && l.id === "approvals" ? { badge: { count: 3, tone: "bad" } } : {}),
      })),
    },
  ],
  footer: [{ label: "Setup", href: "#setup", icon: "⚙", ...(current === "setup" ? { current: true } : {}) }],
});

const screen = (id, title, current, filled, actions, blocks) => ({
  type: "screen",
  id,
  title,
  ...(actions.length ? { actions } : {}),
  blocks: [nav(current, filled), ...blocks],
});

const row = (id, title, subtitle, value, extra = {}) => ({ type: "row", id, title, ...(subtitle ? { subtitle } : {}), ...(value ? { value } : {}), ...extra });
const list = (id, rows) => ({ type: "list", id, rows });
const section = (id, header, blocks, extra = {}) => ({ type: "section", id, header, blocks, ...extra });
const note = (id, markdown) => ({ type: "text", id, markdown });
const button = (id, label, action, style) => ({ type: "button", id, label, action, ...(style ? { style } : {}) });
const tiles = (id, items) => ({ type: "stat", id, items });
const chips = (id, name, options, selected) => ({ type: "chips", id, name, action: name, options: options.map((o) => ({ value: o.toLowerCase(), label: o })), selected: [selected.toLowerCase()] });
const badge = (text, tone) => ({ text, tone });
const slot = (id, blocks) => section(id, "Example pack", blocks, { footer: "A pack slot: filled by an installed pack, absent with none." });

const crews = ["Crew A", "Crew B", "Crew C", "Crew D"];
const holidays = ["2026-12-11"];

const photo = (label) =>
  `data:image/svg+xml,${encodeURIComponent(`<svg xmlns="http://www.w3.org/2000/svg" width="320" height="240"><rect width="320" height="240" fill="#8a8f96"/><path d="M0 190 L90 120 L150 170 L220 100 L320 180 L320 240 L0 240Z" fill="#6c7179"/><text x="16" y="34" font-family="sans-serif" font-size="20" fill="#ffffff">${label}</text></svg>`)}`;

const today = {
  name: "today",
  dark: true,
  menu: true,
  tall: 780,
  phone: 1250,
  states: {
    empty: screen("today", "Today", "today", false, [], [
      tiles("figures", [
        { label: "jobs today", value: 0 },
        { label: "crews out", value: 0 },
        { label: "reports due", value: 0 },
        { label: "waiting on you", value: 0 },
      ]),
      section("mine", "My jobs today", [list("mine-rows", []), note("mine-none", "No job is planned for you today.")]),
      section("due", "Reports due", [list("due-rows", []), note("due-none", "No daily report is due.")]),
      section("waiting", "Waiting on you", [list("waiting-rows", []), note("waiting-none", "Nothing waits for your approval.")], { side: true }),
      {
        type: "card",
        id: "start",
        title: "Set up your organisation",
        subtitle: "Add sites, people and crews, then plan the first job.",
        buttons: [button("open-setup", "Open setup", "open-setup", "primary")],
      },
    ]),
    filled: screen("today", "Today", "today", true, [{ action: "new-report", label: "Write a report" }], [
      tiles("figures", [
        { label: "jobs today", value: 6 },
        { label: "crews out", value: 4 },
        { label: "reports due", value: 2, tone: "warn" },
        { label: "waiting on you", value: 3, tone: "needs" },
      ]),
      section("mine", "My jobs today", [
        list("mine-rows", [
          row("j1042", "Install, Bay 4", "Harbour depot refit · Crew A", "07:30", { badge: badge("on site", "working"), tap: { open: "/jobs/1042" } }),
          row("j1047", "Site survey", "Elm Street housing · Crew B", "09:00", { badge: badge("on site", "working"), tap: { open: "/jobs/1047" } }),
          row("j1051", "Inspection", "Civic library upgrade · Crew C", "13:00", { badge: badge("planned", "info"), tap: { open: "/jobs/1051" } }),
          row("j1053", "Handover walk", "Riverside clinic · Crew D", "15:30", { badge: badge("planned", "info"), tap: { open: "/jobs/1053" } }),
        ]),
      ]),
      section("due", "Reports due", [
        list("due-rows", [
          row("d1", "Install, Bay 4", "Crew A", "17:00", { badge: badge("due", "info"), tap: { open: "/reports/new?job=1042" }, accessory: "chevron" }),
          row("d2", "Strip-out, Block A", "Crew B", "yesterday", { badge: badge("late", "warn"), tap: { open: "/reports/new?job=1046" }, accessory: "chevron" }),
        ]),
      ]),
      slot("pack-today", [list("pack-today-rows", [row("pc1", "Lift check", "Bay 4", "before 10:00", { badge: badge("due", "warn") }), row("pc2", "Access log", "Block A", "08:10", { badge: badge("done", "ok") })])]),
      section("waiting", "Waiting on you", [
        list("waiting-rows", [
          row("w1", "Daily report", "Install, Bay 4 · Ana Silva", "2h", { tap: { open: "/approvals/r881" } }),
          row("w2", "Job order JO-214", "Make good, Level 2", "5h", { tap: { open: "/approvals/o214" } }),
          row("w4", "Daily report", "Site survey · Tomás Reyes", "1d", { tap: { open: "/approvals/r879" } }),
        ]),
      ], { side: true }),
    ]),
  },
};

const projectColumns = [
  { id: "name", label: "Project", kind: "text", phone: "title", width: 22 },
  { id: "client", label: "Client", kind: "text", phone: "meta" },
  { id: "sites", label: "Sites", kind: "number", digits: 0, phone: "hide" },
  { id: "open", label: "Open jobs", kind: "number", digits: 0, phone: "value" },
  { id: "status", label: "Status", kind: "tone", phone: "tag" },
  { id: "start", label: "Start", kind: "date", phone: "hide" },
  { id: "end", label: "End", kind: "date", phone: "hide" },
];
const project = (id, name, client, sites, open, status, tone, start, end) => ({ id, cells: { name, client, sites, open, status: { text: status, tone }, start, end }, tap: { open: `/projects/${id}` } });

const projects = {
  name: "projects",
  tall: 660,
  phone: 1500,
  states: {
    empty: screen("projects", "Projects and sites", "projects", false, [{ action: "new-project", label: "New project" }], [
      chips("which", "which", ["Active", "Planned", "Done", "All"], "Active"),
      { type: "table", id: "projects", title: "Projects", columns: projectColumns, rows: [], empty: "No projects yet. A project groups the sites and jobs of one piece of work.", filter: true },
      section("sites", "Sites", [list("site-rows", []), note("sites-none", "Pick a project to see its sites."), button("new-site", "New site", "new-site")], { side: true }),
    ]),
    filled: screen("projects", "Projects and sites", "projects", true, [{ action: "new-project", label: "New project" }], [
      chips("which", "which", ["Active", "Planned", "Done", "All"], "Active"),
      {
        type: "table",
        id: "projects",
        title: "Projects",
        columns: projectColumns,
        rows: [
          project("p1", "Harbour depot refit", "Port Authority", 3, 7, "on track", "ok", "2026-10-05", "2027-02-26"),
          project("p2", "Elm Street housing", "Elm Street Trust", 2, 5, "behind", "warn", "2026-09-14", "2027-04-30"),
          project("p3", "Civic library upgrade", "Town council", 1, 3, "on track", "ok", "2026-11-02", "2027-01-29"),
          project("p4", "Riverside clinic", "Riverside Health", 1, 2, "handover", "info", "2026-06-01", "2026-12-18"),
          project("p5", "Station car park", "Transit board", 2, 0, "planned", "mute", "2027-01-11", "2027-03-19"),
        ],
        sort: { by: "start", dir: "asc" },
        filter: true,
        export: ["csv"],
      },
      section("sites", "Harbour depot refit", [
        tiles("p1-figures", [
          { label: "sites", value: 3 },
          { label: "open jobs", value: 7 },
          { label: "done", value: "64%" },
        ]),
        list("site-rows", [
          row("s1", "Bay 4", "2 Harbour Road", "3 jobs", { tap: { open: "/sites/s1" }, accessory: "chevron" }),
          row("s2", "Bay 6", "2 Harbour Road", "2 jobs", { tap: { open: "/sites/s2" }, accessory: "chevron" }),
          row("s3", "Gatehouse", "1 Harbour Road", "2 jobs", { tap: { open: "/sites/s3" }, accessory: "chevron" }),
        ]),
        button("new-site", "New site", "new-site"),
        slot("pack-site", [list("pack-site-rows", [row("ps1", "Access hours", null, "07:00 to 17:00"), row("ps2", "Induction needed", null, "yes")])]),
      ], { side: true }),
    ]),
  },
};

const tracks = [
  {
    id: "ca",
    title: "Crew A",
    subtitle: "Ana Silva · 4",
    bars: [
      { id: "b1", start: "2026-11-30", end: "2026-12-04", label: "Install, Bay 4", progress: 60, tone: "accent" },
      { id: "b2", start: "2026-12-07", end: "2026-12-10", label: "Install, Bay 6" },
    ],
  },
  {
    id: "cb",
    title: "Crew B",
    subtitle: "Tomás Reyes · 3",
    bars: [
      { id: "b3", start: "2026-11-30", end: "2026-12-01", label: "Strip-out, Block A", tone: "warn", progress: 70 },
      { id: "b4", start: "2026-12-02", end: "2026-12-02", label: "Survey" },
      { id: "b5", start: "2026-12-08", end: "2026-12-10", label: "Prep, Block B" },
    ],
  },
  {
    id: "cc",
    title: "Crew C",
    subtitle: "Mei Tanaka · 4",
    bars: [
      { id: "b6", start: "2026-12-01", end: "2026-12-03", label: "Inspection, Library", progress: 30 },
      { id: "b7", start: "2026-12-09", end: "2026-12-10", label: "Make good, Level 2", tone: "info" },
    ],
  },
  {
    id: "cd",
    title: "Crew D",
    subtitle: "Kwame Mensah · 2",
    bars: [
      { id: "b8", start: "2026-11-30", end: "2026-11-30", label: "Handover", tone: "ok", progress: 100, fixed: true },
      { id: "b9", start: "2026-12-03", end: "2026-12-05", label: "Snag list" },
    ],
  },
  { id: "un", title: "Unassigned", subtitle: "2 jobs", bars: [{ id: "b10", start: "2026-12-07", end: "2026-12-08", label: "Gatehouse repair", tone: "bad" }, { id: "b11", start: "2026-12-12", end: "2026-12-13", label: "Survey, Bay 6", tone: "bad" }] },
];
const gestures = { move: { action: "schedule", name: "bar" }, stretch: { action: "schedule", name: "bar" }, draw: { action: "schedule", name: "bar" } };
const timeline = (filled) => ({ type: "timeline", id: "plan", title: "30 Nov to 13 Dec · Fri 11 Dec is a holiday", from: "2026-11-30", to: "2026-12-13", off: holidays, tracks: filled ? tracks : [], empty: "No crews yet, so nothing to plan. Add a crew, then drag across its row to plan a job.", ...gestures });
const views = (v) => chips("view", "view", ["Timeline", "Month", "Table"], v);
const plannerFigures = (filled) =>
  tiles("figures", [
    { label: "jobs", value: filled ? 14 : 0 },
    { label: "unassigned", value: filled ? 2 : 0, ...(filled ? { tone: "bad" } : {}) },
    { label: "crews booked", value: filled ? "4 of 4" : "0 of 0" },
  ]);

const plannerTimeline = {
  name: "planner-timeline",
  dark: true,
  tall: 620,
  phone: 1000,
  states: {
    empty: screen("planner", "Planner", "planner", false, [{ action: "new-job", label: "New job" }], [views("Timeline"), plannerFigures(false), timeline(false)]),
    filled: screen("planner", "Planner", "planner", true, [{ action: "new-job", label: "New job" }], [views("Timeline"), plannerFigures(true), timeline(true)]),
  },
};

const jobFields = (v) => [
  { type: "text", name: "title", label: "Job", value: v.title ?? "", placeholder: "What is to be done" },
  { type: "picker", name: "site", label: "Site", options: ["Pick a site", "Bay 4", "Bay 6", "Gatehouse"], value: "Pick a site" },
  { type: "picker", name: "crew", label: "Crew", options: ["Pick a crew", ...crews], value: "Pick a crew" },
  { type: "text", name: "start", label: "Start", value: v.start ?? "", placeholder: "day" },
  { type: "text", name: "end", label: "End", value: v.end ?? "", placeholder: "day" },
];

const jobHours = section("hours", "Hours", [
  tiles("hours-figures", [
    { label: "budget, person-hours", value: "160 h" },
    { label: "used", value: "84.5 h" },
    { label: "left", value: "75.5 h", tone: "ok" },
  ]),
  {
    type: "chart",
    id: "hours-days",
    chart: "combo",
    title: "Person-hours by day",
    format: "count",
    x: ["Mon 30", "Tue 1", "Wed 2", "Thu 3", "Fri 4"],
    series: [
      { name: "used", as: "bar", values: [31, 22.5, 31, 0, 0] },
      { name: "budget", as: "line", dash: true, tone: "mute", values: [32, 32, 32, 32, 32] },
    ],
  },
  section("hours-people", "By person", [
    list("hours-people-rows", [
      row("hp1", "Ana Silva", "3 days", "24 of 40 h"),
      row("hp2", "Ben Okafor", "3 days", "24 of 40 h"),
      row("hp3", "Lena Kovac", "3 days", "21.5 of 40 h"),
      row("hp4", "Sam Ito", "2 days", "15 of 40 h"),
    ]),
  ]),
], { side: true });

const plannerJob = {
  name: "planner-job",
  css: { filled: ["picked.css"] },
  tall: 1560,
  phone: 2050,
  states: {
    empty: screen("planner-job", "Planner", "planner", true, [{ action: "new-job", label: "New job" }], [
      views("Timeline"),
      timeline(true),
      section("job", "New job", [
        { type: "form", id: "job-form", action: "create-job", fields: jobFields({}) },
        button("plan-job", "Plan job", "create-job", "primary"),
        note("job-hint", "Equipment, orders, reports and hours appear here once the job is planned."),
      ], { side: true }),
    ]),
    filled: screen("planner-job", "Planner", "planner", true, [{ action: "new-job", label: "New job" }], [
      views("Timeline"),
      timeline(true),
      section("job", "Install, Bay 4", [
        { type: "steps", id: "job-steps", current: "site", steps: [{ id: "plan", label: "Planned" }, { id: "order", label: "Ordered", sub: "JO-211" }, { id: "site", label: "On site", sub: "since Mon" }, { id: "report", label: "Reported" }, { id: "ok", label: "Approved" }] },
        list("job-facts", [
          row("f1", "Project", null, "Harbour depot refit"),
          row("f2", "Site", null, "Bay 4"),
          row("f3", "Crew", null, "Crew A · 4"),
          row("f4", "Days", null, "30 Nov to 4 Dec"),
          row("f5", "Equipment", null, "Lift L-2, Van V-1"),
          row("f6", "Reports", null, "3 of 5", { tap: { open: "/jobs/1042/reports" }, accessory: "chevron" }),
        ]),
        slot("pack-job", [{ type: "form", id: "pack-job-form", action: "pack-fields", fields: [{ type: "picker", name: "level", label: "Check level", options: ["Basic", "Full"], value: "Full" }, { type: "toggle", name: "permit", label: "Permit on site", value: true }] }]),
        button("open-order", "Open job order", "open-order"),
        button("edit-job", "Edit job", "edit-job", "primary"),
      ], { side: true }),
      jobHours,
    ]),
  },
};

const days = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
const load = [
  [3, 3, 3, 3, 2, 1, 0],
  [2, 3, 3, 3, "off", 1, 1],
  [2, 3, 4, 4, 3, 1, 0],
  [3, 2, 2, 1, 1, 0, 0],
  [0, 1, 2, 2, 0, 0, 0],
];
const monthDay = (filled) =>
  section("day", filled ? "Wed 9 Dec · 3 jobs" : "This month", filled
    ? [list("day-rows", [
        row("m1", "Install, Bay 6", "Crew A", "4 days", { tap: { open: "/jobs/1044" } }),
        row("m2", "Prep, Block B", "Crew B", "3 days", { tap: { open: "/jobs/1049" } }),
        row("m3", "Make good, Level 2", "Crew C", "2 days", { tap: { open: "/jobs/1052" } }),
      ])]
    : [list("day-rows", []), note("day-none", "Pick a day to see its jobs.")], { side: true });

const plannerMonth = {
  name: "planner-month",
  css: { filled: ["month.css"] },
  tall: 620,
  phone: 1000,
  states: {
    empty: screen("planner-month", "Planner", "planner", false, [{ action: "new-job", label: "New job" }], [
      views("Month"),
      { type: "heat", id: "month", title: "December 2026: jobs a day", columns: days, grid: [], empty: "No jobs this month." },
      monthDay(false),
    ]),
    filled: screen("planner-month", "Planner", "planner", true, [{ action: "new-job", label: "New job" }], [
      views("Month"),
      {
        type: "heat",
        id: "month",
        title: "December 2026: jobs a day",
        columns: days,
        grid: load.map((w, i) => ({ name: `${["30 Nov", "7 Dec", "14 Dec", "21 Dec", "28 Dec"][i]}`, cells: w.map((v) => (typeof v === "string" ? { label: v } : { value: v, label: String(v) })) })),
        scale: { min: 0, max: 5 },
        selected: { row: "7 Dec", column: "Wed" },
        tap: { action: "pick-day" },
      },
      monthDay(true),
    ]),
  },
};

const jobColumns = [
  { id: "job", label: "Job", kind: "text", phone: "title", width: 20 },
  { id: "project", label: "Project", kind: "text", phone: "meta" },
  { id: "site", label: "Site", kind: "text", phone: "hide" },
  { id: "crew", label: "Crew", kind: "text", phone: "value" },
  { id: "start", label: "Start", kind: "date", phone: "hide" },
  { id: "end", label: "End", kind: "date", phone: "hide" },
  { id: "status", label: "Status", kind: "tone", phone: "tag" },
  { id: "done", label: "Done", kind: "bar", unit: "%", digits: 0, max: 100, phone: "hide" },
];
const job = (id, j, project, site, crew, start, end, status, tone, done) => ({ id, cells: { job: j, project, site, crew, start, end, status: { text: status, tone }, done }, tap: { open: `/jobs/${id}` } });

const plannerTable = {
  name: "planner-table",
  tall: 560,
  phone: 1250,
  states: {
    empty: screen("planner-table", "Planner", "planner", false, [{ action: "new-job", label: "New job" }], [
      views("Table"),
      { type: "table", id: "jobs", title: "Jobs", columns: jobColumns, rows: [], empty: "No jobs yet. Plan one on the timeline, or import them from a spreadsheet.", filter: true },
    ]),
    filled: screen("planner-table", "Planner", "planner", true, [{ action: "new-job", label: "New job" }], [
      views("Table"),
      {
        type: "table",
        id: "jobs",
        title: "Jobs",
        columns: jobColumns,
        rows: [
          job("1042", "Install, Bay 4", "Harbour depot refit", "Bay 4", "Crew A", "2026-11-30", "2026-12-04", "on site", "working", 60),
          job("1044", "Install, Bay 6", "Harbour depot refit", "Bay 6", "Crew A", "2026-12-07", "2026-12-10", "planned", "info", 0),
          job("1046", "Strip-out, Block A", "Elm Street housing", "Block A", "Crew B", "2026-11-30", "2026-12-01", "behind", "warn", 70),
          job("1049", "Prep, Block B", "Elm Street housing", "Block B", "Crew B", "2026-12-08", "2026-12-10", "planned", "info", 0),
          job("1051", "Inspection, Library", "Civic library upgrade", "Main hall", "Crew C", "2026-12-01", "2026-12-03", "on site", "working", 30),
          job("1053", "Handover", "Riverside clinic", "Ward wing", "Crew D", "2026-11-30", "2026-11-30", "done", "ok", 100),
          job("1055", "Gatehouse repair", "Harbour depot refit", "Gatehouse", "", "2026-12-07", "2026-12-08", "no crew", "bad", 0),
        ],
        sort: { by: "start", dir: "asc" },
        filter: true,
        select: { actions: [{ action: "assign", label: "assign crew" }, { action: "shift", label: "move days" }] },
        export: ["csv", "xlsx"],
      },
    ]),
  },
};

const equipColumns = [
  { id: "item", label: "Item", kind: "text", phone: "title" },
  { id: "kind", label: "Kind", kind: "text", phone: "meta" },
  { id: "where", label: "Where", kind: "text", phone: "value" },
  { id: "status", label: "Status", kind: "tone", phone: "tag" },
  { id: "next", label: "Next booked", kind: "date", phone: "hide" },
];
const equip = (id, item, kind, where, status, tone, next) => ({ id, cells: { item, kind, where, status: { text: status, tone }, ...(next ? { next } : {}) }, tap: { open: `/equipment/${id}` } });

const crew = {
  name: "crew",
  tall: 720,
  phone: 2000,
  states: {
    empty: screen("crew", "Crew and equipment", "crew", false, [{ action: "import", label: "Import" }], [
      section("crews", "Crews", [list("crew-rows", []), note("crews-none", "No crews yet. A crew is the people who go out together."), button("new-crew", "New crew", "new-crew", "primary")]),
      { type: "table", id: "equipment", title: "Equipment", columns: equipColumns, rows: [], empty: "No equipment yet. Add what you book onto jobs." },
      section("people", "People", [list("people-rows", []), note("people-none", "No one is invited yet."), button("invite", "Invite people", "invite")], { side: true }),
    ]),
    filled: screen("crew", "Crew and equipment", "crew", true, [{ action: "import", label: "Import" }], [
      section("crews", "Crews", [
        list("crew-rows", [
          row("ca", "Crew A", "Lead Ana Silva · 4 people", "Bay 4", { badge: badge("on site", "working"), tap: { open: "/crews/ca" } }),
          row("cb", "Crew B", "Lead Tomás Reyes · 3 people", "Block A", { badge: badge("on site", "working"), tap: { open: "/crews/cb" } }),
          row("cc", "Crew C", "Lead Mei Tanaka · 4 people", "Main hall", { badge: badge("on site", "working"), tap: { open: "/crews/cc" } }),
          row("cd", "Crew D", "Lead Kwame Mensah · 2 people", "yard", { badge: badge("free", "mute"), tap: { open: "/crews/cd" } }),
        ]),
      ]),
      {
        type: "table",
        id: "equipment",
        title: "Equipment",
        columns: equipColumns,
        rows: [
          equip("L-2", "Lift L-2", "Lift", "Bay 4", "on site", "working", "2026-12-07"),
          equip("V-1", "Van V-1", "Van", "Bay 4", "on site", "working", "2026-12-07"),
          equip("G-3", "Generator G-3", "Generator", "yard", "free", "ok", "2026-12-09"),
          equip("S-1", "Scaffold set S-1", "Access", "Block A", "on site", "working"),
          equip("L-1", "Lift L-1", "Lift", "workshop", "out of service", "bad"),
        ],
        filter: true,
      },
      section("people", "People", [
        list("people-rows", [
          row("u1", "Ana Silva", "Lead · Crew A"),
          row("u2", "Tomás Reyes", "Lead · Crew B"),
          row("u3", "Mei Tanaka", "Lead · Crew C"),
          row("u4", "Kwame Mensah", "Lead · Crew D"),
          row("u5", "Priya Nair", "Planner"),
        ]),
        button("invite", "Invite people", "invite"),
        slot("pack-crew", [list("pack-crew-rows", [row("pk1", "Lift ticket", "2 people", "ends in 30 days", { badge: badge("soon", "warn") })])]),
      ], { side: true }),
    ]),
  },
};

const reportForm = (v) => [
  { type: "picker", name: "job", label: "Job", options: v.job ? ["Install, Bay 4", "Strip-out, Block A", "Inspection, Library"] : ["Pick a job"], value: v.job ?? "Pick a job" },
  { type: "text", name: "day", label: "Day", value: v.day ?? "Tue 1 Dec 2026" },
];
const reportPeople = (items) => ({
  type: "lines",
  id: "people",
  name: "people",
  label: "Who was on site",
  add: "Add person",
  columns: [
    { type: "picker", name: "person", label: "Person", options: ["Ana Silva", "Ben Okafor", "Lena Kovac", "Sam Ito"] },
    { type: "text", name: "hours", label: "Hours", keyboard: "number" },
  ],
  items,
  sum: ["hours"],
  empty: "No one added yet.",
});

const reportBlocks = (filled) => [
  { type: "form", id: "report-head", action: "report-head", fields: reportForm(filled ? { job: "Install, Bay 4" } : {}) },
  chips("conditions", "conditions", ["Clear", "Rain", "Wind", "Heat"], filled ? "Rain" : ""),
  reportPeople(filled
    ? [
        { id: "p1", values: { person: "Ana Silva", hours: "8" } },
        { id: "p2", values: { person: "Ben Okafor", hours: "8" } },
        { id: "p3", values: { person: "Lena Kovac", hours: "6.5" } },
      ]
    : []),
  {
    type: "form",
    id: "report-photos",
    action: "add-photos",
    fields: [{ type: "upload", name: "photos", label: "Photos", many: true, capture: true, accept: ["image"], files: filled ? [{ id: "f1", name: "north-wall.jpg", src: photo("North wall") }, { id: "f2", name: "frames.jpg", src: photo("Frames") }] : [] }],
  },
  {
    type: "form",
    id: "report-body",
    action: "send-report",
    fields: [
      { type: "text", name: "done", label: "Work done", value: filled ? "Fixed the last two frames on the north wall." : "", placeholder: "What the crew did" },
      { type: "text", name: "issues", label: "Delays or issues", value: filled ? "Rain stopped work 10:00 to 11:30." : "", placeholder: "What held the work up" },
      { type: "toggle", name: "safe", label: "Site left safe", value: filled },
    ],
  },
  button("send", "Send for approval", "send-report", "primary"),
];

const report = {
  name: "report",
  tall: 900,
  phone: 1400,
  states: {
    empty: screen("report", "Daily report", "reports", false, [{ action: "save-draft", label: "Save draft" }], [
      ...reportBlocks(false),
      section("earlier", "Earlier reports", [list("earlier-rows", []), note("earlier-none", "Reports you send appear here.")], { side: true }),
    ]),
    filled: screen("report", "Daily report", "reports", true, [{ action: "save-draft", label: "Save draft" }], [
      ...reportBlocks(true),
      slot("pack-report", [{ type: "form", id: "pack-report-form", action: "pack-fields", fields: [{ type: "picker", name: "check", label: "End-of-day check", options: ["Passed", "Failed", "Not done"], value: "Passed" }, { type: "text", name: "reading", label: "Reading", keyboard: "number", value: "42" }] }]),
      section("earlier", "Earlier reports", [
        list("earlier-rows", [
          row("r881", "Mon 30 Nov", "Install, Bay 4", null, { badge: badge("approved", "ok") }),
          row("r874", "Fri 27 Nov", "Install, Bay 4", null, { badge: badge("returned", "warn") }),
          row("r866", "Thu 26 Nov", "Prep, Bay 4", null, { badge: badge("approved", "ok") }),
        ]),
      ], { side: true }),
    ]),
  },
};

const orderColumns = [
  { id: "no", label: "Order", kind: "text", phone: "meta" },
  { id: "job", label: "Job", kind: "text", phone: "title", width: 20 },
  { id: "site", label: "Site", kind: "text", phone: "hide" },
  { id: "to", label: "Issued to", kind: "text", phone: "value" },
  { id: "due", label: "Due", kind: "date", phone: "hide" },
  { id: "status", label: "Status", kind: "tone", phone: "tag" },
];
const order = (no, j, site, to, due, status, tone) => ({ id: no, cells: { no, job: j, site, to, due, status: { text: status, tone } }, tap: { open: `/orders/${no}` } });

const orders = {
  name: "orders",
  tall: 720,
  phone: 1150,
  states: {
    empty: screen("orders", "Job orders", "orders", false, [{ action: "new-order", label: "New order" }], [
      chips("which", "which", ["Open", "Draft", "Done", "All"], "Open"),
      { type: "table", id: "orders", title: "Job orders", columns: orderColumns, rows: [], empty: "No job orders yet. An order tells a crew what to do on a job, and prints for the site." },
      section("order", "Order", [note("order-none", "Pick an order to see it.")], { side: true }),
    ]),
    filled: screen("orders", "Job orders", "orders", true, [{ action: "new-order", label: "New order" }], [
      chips("which", "which", ["Open", "Draft", "Done", "All"], "Open"),
      {
        type: "table",
        id: "orders",
        title: "Job orders",
        columns: orderColumns,
        rows: [
          order("JO-211", "Install, Bay 4", "Bay 4", "Crew A", "2026-12-04", "accepted", "working"),
          order("JO-212", "Strip-out, Block A", "Block A", "Crew B", "2026-12-01", "accepted", "working"),
          order("JO-214", "Make good, Level 2", "Main hall", "Crew C", "2026-12-10", "to approve", "needs"),
          order("JO-215", "Install, Bay 6", "Bay 6", "Crew A", "2026-12-10", "issued", "info"),
          order("JO-216", "Gatehouse repair", "Gatehouse", "no crew", "2026-12-08", "draft", "mute"),
        ],
        sort: { by: "due", dir: "asc" },
        filter: true,
      },
      section("order", "JO-211 · Install, Bay 4", [
        { type: "steps", id: "order-steps", current: "accepted", steps: [{ id: "draft", label: "Draft" }, { id: "issued", label: "Issued", sub: "27 Nov" }, { id: "accepted", label: "Accepted", sub: "Ana Silva" }, { id: "done", label: "Done" }] },
        note("scope", "**Scope.** Fix the frames along the north wall of Bay 4 and leave the bay clear for the next crew."),
        list("order-facts", [row("o1", "Crew", null, "Crew A"), row("o2", "Days", null, "30 Nov to 4 Dec"), row("o3", "Equipment", null, "Lift L-2, Van V-1")]),
        slot("pack-order", [list("pack-order-rows", [row("po1", "Checks to do", null, "Lift check, Access log")])]),
        button("print", "Print", "print"),
        button("done", "Mark done", "mark-done", "primary"),
      ], { side: true }),
    ]),
  },
};

const ask = (id, title, subtitle, facts, extra = []) => ({
  type: "card",
  id,
  kind: "ask",
  title,
  subtitle,
  blocks: [list(`${id}-facts`, facts.map(([t, v], i) => row(`${id}-f${i}`, t, null, v))), ...extra],
  buttons: [button(`${id}-ok`, "Approve", "approve", "primary"), button(`${id}-back`, "Return", "return")],
});

const approvals = {
  name: "approvals",
  tall: 940,
  phone: 1900,
  states: {
    empty: screen("approvals", "Approvals", "approvals", false, [], [
      chips("which", "which", ["All", "Reports", "Orders"], "All"),
      note("none", "Nothing waits for your approval. Daily reports and job orders sent to you appear here."),
      section("decided", "Decided", [list("decided-rows", []), note("decided-none", "Nothing decided yet.")], { side: true }),
    ]),
    filled: screen("approvals", "Approvals", "approvals", true, [], [
      chips("which", "which", ["All", "Reports", "Orders"], "All"),
      ask("r881", "Daily report · Install, Bay 4", "Ana Silva · Tue 1 Dec", [["People", "3 · 22.5 h"], ["Work done", "Frames on the north wall"], ["Issues", "Rain 10:00 to 11:30"], ["Photos", "2"]]),
      ask("o214", "Job order JO-214 · Make good, Level 2", "Priya Nair · for Crew C", [["Days", "9 to 10 Dec"], ["Site", "Main hall"]]),
      section("decided", "Decided", [
        list("decided-rows", [
          row("x1", "Daily report · Survey", "approved by you", "Mon", { badge: badge("approved", "ok") }),
          row("x2", "Daily report · Install, Bay 4", "returned: no photos", "Fri", { badge: badge("returned", "warn") }),
        ]),
      ], { side: true }),
    ]),
  },
};

const setupSteps = (current) => ({ type: "steps", id: "setup-steps", title: "Set up", current, steps: [{ id: "org", label: "Organisation" }, { id: "sites", label: "Sites" }, { id: "people", label: "People" }, { id: "crews", label: "Crews" }, { id: "packs", label: "Packs" }] });
const orgForm = (v) => ({
  type: "config",
  id: "org",
  action: "save-org",
  fields: [
    { type: "text", name: "name", label: "Organisation name", value: v.name ?? "", placeholder: "Your organisation" },
    { type: "picker", name: "zone", label: "Time zone", options: v.zone ? ["UTC-05:00", "UTC+00:00", "UTC+01:00", "UTC+07:00"] : ["Pick a time zone"], value: v.zone ?? "Pick a time zone" },
    { type: "picker", name: "week", label: "Week starts", options: ["Monday", "Sunday", "Saturday"], value: v.week ?? "Monday" },
    { type: "text", name: "day", label: "Working day", value: v.day ?? "", placeholder: "07:00 to 17:00" },
  ],
});

const setup = {
  name: "setup",
  tall: 950,
  phone: 1250,
  states: {
    empty: screen("setup", "Setup", "setup", false, [], [
      setupSteps("org"),
      orgForm({}),
      { type: "chips", id: "workdays", name: "workdays", action: "workdays", multi: true, options: days.map((d) => ({ value: d.toLowerCase(), label: d })), selected: [] },
      button("save-org", "Save", "save-org", "primary"),
      section("roles", "Roles", [list("role-rows", [row("ro1", "Admin", "Everything, setup included", "1"), row("ro2", "Planner", "Plans jobs, issues orders, approves", "0"), row("ro3", "Lead", "Runs a crew, writes reports", "0"), row("ro4", "Member", "Sees own jobs, writes reports", "0")])]),
      section("packs", "Packs", [list("pack-rows", []), note("packs-none", "No pack installed. openworks runs complete without one; a pack adds its own words, fields and screens.")], { side: true }),
      section("import", "Import", [note("import-note", "Bring sites, people and equipment from a spreadsheet."), button("import", "Import a spreadsheet", "import")], { side: true }),
    ]),
    filled: screen("setup", "Setup", "setup", true, [], [
      setupSteps("packs"),
      orgForm({ name: "Northgate Works", zone: "UTC+07:00", week: "Monday", day: "07:00 to 17:00" }),
      { type: "chips", id: "workdays", name: "workdays", action: "workdays", multi: true, options: days.map((d) => ({ value: d.toLowerCase(), label: d })), selected: ["mon", "tue", "wed", "thu", "fri"] },
      button("save-org", "Save", "save-org", "primary"),
      section("roles", "Roles", [list("role-rows", [row("ro1", "Admin", "Everything, setup included", "2"), row("ro2", "Planner", "Plans jobs, issues orders, approves", "3"), row("ro3", "Lead", "Runs a crew, writes reports", "4"), row("ro4", "Member", "Sees own jobs, writes reports", "9")])]),
      section("packs", "Packs", [list("pack-rows", [row("pk", "Example pack", "Adds checks to Today, fields to jobs, sites and reports", "0.1.0", { badge: badge("on", "ok") })])]),
      section("import", "Import", [note("import-note", "Last import: 18 people, 5 pieces of equipment, 24 Nov."), button("import", "Import a spreadsheet", "import")], { side: true }),
    ]),
  },
};

export const screens = [today, projects, plannerTimeline, plannerJob, plannerMonth, plannerTable, crew, report, orders, approvals, setup];
