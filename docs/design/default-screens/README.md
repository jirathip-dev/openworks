## Summary

Decision: every default screen is a Blocks screen, built only from Blocks' own parts and shown in a new openworks colour pair (light and dark) and style. Blocks' renderer drew every image, and the colours pass the Blocks contrast check. Pack slots show up only when a pack fills them.
Screens: Today; projects and sites; the planner as a timeline, with a job's panel, as a month and as a table; crew and equipment; the daily report (phone first); job orders; approvals; setup. Each is drawn empty and filled, at 1440 and 390.
Open questions: (1) "Slots" is read here as two things: an empty screen has no records and no pack, and a filled one has sample records plus the example pack filling its slots. (2) Blocks has no calendar part, so the month view is a jobs-per-day heat grid next to the picked day's jobs. Is that enough, or should Blocks get a calendar part? (3) Should the openworks pair go into swag beside the other schemes, or stay in openworks?

## How it is made

- `screens.mjs` builds each screen as Blocks screen data, in an empty and a filled state, and `screens/` holds the result (`<screen>.<state>.json`). Impl can turn these into the app's builders and guard fixtures.
- `look.mjs` holds the openworks pair and style. `build.mjs` checks the pair with Blocks' own gate (`gated`), writes `openworks.css` with Blocks' `css`, then draws every screen with Blocks' headless harness (`web/shot/cli.ts`, WebKit) into `png/`.
- To redraw: `BLOCKS=<a blocks checkout with web/node_modules> node docs/design/default-screens/build.mjs`. This set was drawn at blocks `127042f9b`.
- `openworks.css` also has two layout lines that stand in for Blocks' nav layout: the sidebar is a rail at the left on wide screens and a menu bar on phones. In the product, the layout's `side` slot places it, so these lines are not product CSS.

## The look

| | Light | Dark |
|---|---|---|
| page, card | `#f3f2ee`, `#ffffff` (concrete grey, white) | `#14171a`, `#1c2024` |
| text, muted | `#1b1e22`, `#585d64` | `#e9e7e2`, `#a5abb2` |
| ink and links | `#1d4d8f` (plan blue) | `#8db6f2` |
| ok, warn, bad | `#2e7a3c`, `#9a5c00`, `#b4271f` | `#6cbf78`, `#e3a43c`, `#f08a80` |

Style (`openworks-style.json`): system sans type, solid 2px edges, 6px corners, filled primary buttons, 10% more space for gloved taps, and the calm easing. The full token files are `openworks-light.json` and `openworks-dark.json`.

## Every screen

Navigation is the same on every screen: a rail with Today, Planner, Projects and sites, Job orders, Daily reports, Approvals, and Crew and equipment, with Setup at the foot. On a phone it becomes a menu. Approvals carries a count of what waits on you.

| Image (in `png/`, at `-1440` and `-390`) | Decision it shows |
|---|---|
| `today-empty`, `today-filled`, `today-filled-dark` | Today opens on four figures (jobs, crews out, reports due, waiting on you), then my jobs and the reports due, with what waits on me beside them. An empty org gets one "Set up your organisation" card. |
| `projects-empty`, `projects-filled` | Projects are a table you can filter, sort and export. The picked project's sites sit in a side panel with its figures, plus a site's pack fields. |
| `planner-timeline-empty`, `planner-timeline-filled`, `planner-timeline-filled-dark` | The planner opens on a two-week timeline with a row per crew and an Unassigned row in red. You drag a bar to move a job, stretch it to change its days, or draw on a row to plan a new one. Weekends are shaded. Timeline, month and table are one screen with three views. |
| `planner-job-empty`, `planner-job-filled` | A job opens in a panel beside the timeline, so the plan stays in view. It shows the job's progress as steps (planned, ordered, on site, reported, approved), its facts, and the pack's job fields. With no job picked, the panel is the "New job" form. |
| `planner-month-empty`, `planner-month-filled` | The month is a heat grid of jobs a day, by week. Tapping a day lists that day's jobs beside it. |
| `planner-table-empty`, `planner-table-filled` | The table lists every job with its status and how far along it is. You can pick several to assign a crew or move their days together, and export it. |
| `crew-empty`, `crew-filled` | Crews are listed with where each is today. Equipment is a table showing where each item is and its next booking, with out of service in red. People sit beside them, with the pack's ticket reminders. Everything can be imported from a spreadsheet. |
| `report-empty`, `report-filled` | The daily report is one column, built for a phone: the job and day, conditions, who was on site with their hours (only recorded, never paid), photos straight from the camera, the work done, delays, and "site left safe". Then the pack's end-of-day fields and one "Send for approval" button. Earlier reports sit beside it on wide screens. |
| `orders-empty`, `orders-filled` | Job orders are a table by due day. The picked order shows its steps (draft, issued, accepted, done), its scope, its crew and equipment, and Print, which uses the print templates. |
| `approvals-empty`, `approvals-filled` | Each thing waiting is one card with its key facts and two buttons, Approve and Return. Chips filter by reports, orders or hours. What you already decided is listed beside. |
| `setup-empty`, `setup-filled` | Setup runs in five steps: organisation, sites, people, crews and packs. It holds the organisation settings (name, time zone, week start, working day and days), roles and their head counts, installed packs, and spreadsheet import. With no pack, it says openworks is complete without one. |

The screens hold no money, no trade and no country. Hours are recorded only, and time zones are UTC offsets. "Example pack" stands in for any pack, and its slot footer marks each place a pack can fill.
