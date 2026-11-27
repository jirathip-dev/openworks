const pen = document.createElement("canvas").getContext("2d");
const fit = () => {
  for (const b of document.querySelectorAll(".tmm>span>b")) {
    const full = (b.dataset.full ??= b.textContent);
    const [month, year] = full.split(" ");
    pen.font = getComputedStyle(b).font;
    const room = b.parentElement.clientWidth - 12;
    const best = [full, `${month.slice(0, 3)} ${year}`, month.slice(0, 3)].find((t) => pen.measureText(t).width <= room) ?? "";
    if (b.textContent !== best) b.textContent = best;
  }
};
new MutationObserver(fit).observe(document.documentElement, { childList: true, subtree: true });
addEventListener("resize", fit);
