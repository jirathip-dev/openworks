## Summary

Decision: every default screen is a Blocks screen in the openworks colour pair and style. This round makes the owner's eight changes: hours are entered once, in the daily report, and a job shows them read only (budget against used, by day and by person); there are no hours approvals; weekends get a light shade and holidays a strong one; the timeline is cleaned up and shows the picked job; job steps carry their names; phones get a small working "Menu"; a report has one line per person; and on wide screens the rail is labelled.
Screens: Today; projects and sites; the planner as a timeline, with a job's panel (now with its hours), as a month and as a table; crew and equipment; the daily report (phone first); job orders; approvals; setup. Each is drawn empty and filled, at 1440 and 390, plus the phone menu open.
Open questions: none. The month view stays a heat grid and the colour pair stays in openworks, as decided. The timeline, steps, person-line and rail fixes below are drawn with stand-in CSS here; impl brings them to Blocks.

## How it is made

- `screens.mjs` builds each screen as Blocks screen data, in an empty and a filled state, and `screens/` holds the result (`<screen>.<state>.json`). Impl can turn these into the app's builders and guard fixtures.
- `look.mjs` holds the openworks pair and style. `build.mjs` checks the pair with Blocks' own gate (`gated`), writes `openworks.css` with Blocks' `css`, then draws every screen with Blocks' headless harness (`web/shot/cli.ts`, WebKit) into `png/`.
- `layout.css` is appended to `openworks.css` and holds stand-in lines for what Blocks should do itself, for impl to carry into Blocks: the sidebar as a rail (labelled from 1100 px wide; icons with their names as tooltips below that) and as a small "Menu" under the title on phones; weekends shaded lightly and holidays (the timeline's `off` days) strongly; the day width filling the timeline's whole width; grips only on hover or focus; a one-day bar keeping its name inside the bar; five or more steps listed with their names in a narrow panel; and a person line on one row on phones. `months.js` shortens a month name that does not fit ("Nov 2026", "Nov", or nothing), at every zoom. `picked.css` highlights the open job's bar (Blocks' timeline has no picked bar yet). `menu-open.css` only stills the drawer's slide for its shot.
- To redraw: `BLOCKS=<a blocks checkout with web/node_modules> node docs/design/default-screens/build.mjs`. This set was drawn at blocks `127042f9b`.

## The look

| | Light | Dark |
|---|---|---|
| page, card | `#f3f2ee`, `#ffffff` (concrete grey, white) | `#14171a`, `#1c2024` |
| text, muted | `#1b1e22`, `#585d64` | `#e9e7e2`, `#a5abb2` |
| ink and links | `#1d4d8f` (plan blue) | `#8db6f2` |
| ok, warn, bad | `#2e7a3c`, `#9a5c00`, `#b4271f` | `#6cbf78`, `#e3a43c`, `#f08a80` |

Style (`openworks-style.json`): system sans type, solid 2px edges, 6px corners, filled primary buttons, 10% more space for gloved taps, and the calm easing. The full token files are `openworks-light.json` and `openworks-dark.json`.

## Every screen

Navigation is the same on every screen: a labelled rail with Today, Planner, Projects and sites, Job orders, Daily reports, Approvals, and Crew and equipment, with Setup at the foot. On a phone it is a small "Menu" under the title that opens the same list as a drawer (`today-filled-menu-390`). Approvals carries a count of what waits on you.

| Image (in `png/`, at `-1440` and `-390`) | Decision it shows |
|---|---|
| `today-empty`, `today-filled`, `today-filled-dark` | Today opens on four figures (jobs, crews out, reports due, waiting on you), then my jobs and the reports due, with what waits on me beside them (daily reports and job orders only). An empty org gets one "Set up your organisation" card. |
| `projects-empty`, `projects-filled` | Projects are a table you can filter, sort and export. The picked project's sites sit in a side panel with its figures, plus a site's pack fields. |
| `planner-timeline-empty`, `planner-timeline-filled`, `planner-timeline-filled-dark` | The planner opens on a two-week timeline across the full width, with a row per crew and an Unassigned row in red. You drag a bar to move a job, stretch it at its ends (the grips show on hover) to change its days, or draw on a row to plan a new one. Saturday and Sunday are shaded lightly and take work like any day (Snag list, Survey, Bay 6); only a holiday from the work calendar (Fri 11 Dec) gets the strong hatched shade. A one-day job keeps its name inside its bar. Timeline, month and table are one screen with three views. |
| `planner-job-empty`, `planner-job-filled` | A job opens in a panel beside the timeline, so the plan stays in view, and its bar is highlighted. The panel lists the job's steps by name (planned, ordered, on site, reported, approved), its facts and the pack's job fields. Below them are its hours, read only: the person-hours budget against what is used, by day as a chart and by person as a list, all taken from the daily reports. With no job picked, the panel is the "New job" form. |
| `planner-month-empty`, `planner-month-filled` | The month is a heat grid of jobs a day, by week, with a holiday marked "off". Tapping a day lists that day's jobs beside it. |
| `planner-table-empty`, `planner-table-filled` | The table lists every job with its status and how far along it is. You can pick several to assign a crew or move their days together, and export it. |
| `crew-empty`, `crew-filled` | Crews are listed with where each is today. Equipment is a table showing where each item is and its next booking, with out of service in red. People sit beside them, with the pack's ticket reminders. Everything can be imported from a spreadsheet. |
| `report-empty`, `report-filled` | The daily report is one column, built for a phone: the job and day, conditions, who was on site with their hours, one line a person (name, then hours); this is the one place hours are entered, and they are recorded, never paid, photos straight from the camera, the work done, delays, and "site left safe". Then the pack's end-of-day fields and one "Send for approval" button. Earlier reports sit beside it on wide screens. |
| `orders-empty`, `orders-filled` | Job orders are a table by due day. The picked order shows its steps (draft, issued, accepted, done), its scope, its crew and equipment, and Print, which uses the print templates. |
| `approvals-empty`, `approvals-filled` | Each thing waiting is one card with its key facts and two buttons, Approve and Return. Only daily reports and job orders come here; chips filter by them. Hours are never approved on their own. What you already decided is listed beside. |
| `setup-empty`, `setup-filled` | Setup runs in five steps: organisation, sites, people, crews and packs. It holds the organisation settings (name, time zone, week start, working day and days), roles and their head counts, installed packs, and spreadsheet import. With no pack, it says openworks is complete without one. |

The screens hold no money, no trade and no country. Hours are recorded only, and time zones are UTC offsets. "Example pack" stands in for any pack, and its slot footer marks each place a pack can fill.
