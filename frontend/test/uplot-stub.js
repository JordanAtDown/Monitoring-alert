export default class uPlot {
  constructor(opts, data, host) { this.opts = opts; this.data = data; const d = document.createElement('div'); d.className = 'u-wrap-stub'; d.setAttribute('data-series', String(opts.series.length)); host.append(d); }
  destroy() {}
  setSize() {}
  static tzDate(d) { return d; }
}
