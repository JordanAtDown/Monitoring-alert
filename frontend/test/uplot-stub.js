export default class uPlot {
  constructor(opts, data, host) {
    this.opts = opts;
    this.data = data;
    const d = document.createElement("div");
    d.className = "u-wrap-stub";
    d.setAttribute("data-series", String(opts.series.length));
    d.setAttribute("data-labels", opts.series.slice(1).map((s) => s.label).join("|"));
    host.append(d);
  }
  destroy() {}
  setSize() {}
  static tzDate(d) { return d; }
}
