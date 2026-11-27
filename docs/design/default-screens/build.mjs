import { copyFileSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { execFileSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { pair, style } from "./look.mjs";
import { screens } from "./screens.mjs";

const here = dirname(fileURLToPath(import.meta.url));
const blocks = process.env.BLOCKS;
if (!blocks) {
  process.stderr.write("usage: BLOCKS=<a blocks checkout with web/node_modules> node build.mjs\n");
  process.exit(2);
}

const { gated, css } = await import(join(blocks, "web/src/core/look.ts"));
const look = gated(pair);
writeFileSync(join(here, "openworks.css"), `${css(look, style)}\n${readFileSync(join(here, "layout.css"), "utf8")}`);
writeFileSync(join(here, "openworks-light.json"), JSON.stringify(pair.light, null, 2) + "\n");
writeFileSync(join(here, "openworks-dark.json"), JSON.stringify(pair.dark, null, 2) + "\n");
writeFileSync(join(here, "openworks-style.json"), JSON.stringify(style, null, 2) + "\n");

rmSync(join(here, "screens"), { recursive: true, force: true });
rmSync(join(here, "png"), { recursive: true, force: true });
mkdirSync(join(here, "screens"));
mkdirSync(join(here, "png"));

const out = mkdtempSync(join(tmpdir(), "ow-shots-"));
const runs = [];
const scripts = [join(here, "months.js")];
for (const s of screens) {
  for (const [state, screen] of Object.entries(s.states)) {
    const file = join(here, "screens", `${s.name}.${state}.json`);
    writeFileSync(file, JSON.stringify(screen, null, 2) + "\n");
    for (const scheme of s.dark ? ["light", "dark"] : ["light"]) {
      if (scheme === "dark" && state !== "filled") continue;
      for (const [w, h] of [[1440, s.tall ?? 1000], [390, s.phone ?? 1500]]) {
        const tag = scheme === "dark" ? `-dark` : "";
        const css = [join(here, "openworks.css"), ...(s.css?.[state] ?? []).map((f) => join(here, f))];
        runs.push({ name: `${s.name}-${state}${tag}-${w}`, screen: file, size: `${w}x${h}`, css, scripts, scheme });
        if (s.menu && state === "filled" && scheme === "light" && w === 390) {
          runs.push({ name: `${s.name}-${state}-menu-${w}`, screen: file, size: `${w}x844`, css: [...css, join(here, "menu-open.css")], scripts, scheme, steps: [{ tap: { id: "nav" } }, { shot: { name: "menu" } }] });
        }
      }
    }
  }
}
const plan = join(out, "plan.json");
writeFileSync(plan, JSON.stringify({ out, runs }, null, 2));
execFileSync("node", ["--no-warnings", join(blocks, "web/shot/cli.ts"), "--plan", plan], { stdio: "inherit" });
for (const r of runs) {
  const dir = join(out, r.name);
  const png = readdirSync(dir).find((f) => f.endsWith(".png"));
  copyFileSync(join(dir, png), join(here, "png", `${r.name}.png`));
}
rmSync(out, { recursive: true, force: true });
process.stdout.write(`gate passed; ${runs.length} shots in png/\n`);
