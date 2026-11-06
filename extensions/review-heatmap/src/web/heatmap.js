/* Manatan SRS Review Heatmap calendar renderer.
 *
 * Plain SVG/JS calendar with year/month domains and daily cells:
 * 10px cells, 2px padding, 2px domain gutter, 1px domain margins, and a
 * 25px label band. Styling uses .graph-rect, .qN, .now, .future, .highlight,
 * and .highlight-now. Navigation is clamped to the data bounds:
 * Shift for first/last page, home button rewind, Shift-home mode switch,
 * and Shift-options theme cycle. Day clicks search rid:… for past days
 * and prop:due=N for future days.
 *
 * All day math is done on integer "day indices" (days since epoch, shifted
 * by the host's UTC offset) with a civil-calendar conversion, so the cells
 * match the Rust-side bucketing exactly and are immune to DST quirks.
 */
(function () {
  "use strict";

  var D = window.RH_DATA;
  if (!D) return;
  var root = document.getElementById("review-heatmap-root");
  if (!root) return;

  var CELL = 10;
  var PAD = 2;
  var PITCH = CELL + PAD; // 12
  var LABEL_H = 25; // domainVerticalLabelHeight
  var BLOCK_H = 7 * PITCH + PAD + LABEL_H + 2; // h(d,true) = 113
  var SVG_H = BLOCK_H - 4; // 109
  var SVG_NS = "http://www.w3.org/2000/svg";

  var mobile = isMobileLayout();
  var viewportWidth = root.clientWidth;
  var suppressSwipeClick = false;
  function isMobileLayout() {
    return window.innerWidth < 600 || window.matchMedia("(pointer: coarse)").matches;
  }
  // Keep the saved desktop mode; phones show a shorter run of the same
  // month domains, with exactly the same cell size and rendering code.
  function usesYear() { return !mobile && D.mode === "year"; }
  function visibleRange() {
    if (!mobile) return usesYear() ? 1 : 9;
    // A month occupies at most six week columns. Reserve the weekday band
    // and use a fixed count per viewport so paging forward/back is stable.
    return Math.max(1, Math.min(9, Math.floor((root.clientWidth - PITCH + 4) / (6 * PITCH + 4))));
  }

  var THEMES = ["lime", "olive", "ice", "magenta", "flame"];
  var MODES = ["year", "months"];
  var WEEKDAYS = [
    "Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday",
    "Saturday",
  ];
  var MONTHS = [
    "January", "February", "March", "April", "May", "June", "July",
    "August", "September", "October", "November", "December",
  ];

  // ---- civil calendar math on day indices (days since 1970-01-01) ----

  // Howard Hinnant's civil_from_days / days_from_civil.
  function civilFromDay(z) {
    z += 719468;
    var era = Math.floor(z / 146097);
    var doe = z - era * 146097;
    var yoe = Math.floor(
      (doe - Math.floor(doe / 1460) + Math.floor(doe / 36524) -
        Math.floor(doe / 146096)) / 365,
    );
    var y = yoe + era * 400;
    var doy = doe - (365 * yoe + Math.floor(yoe / 4) - Math.floor(yoe / 100));
    var mp = Math.floor((5 * doy + 2) / 153);
    var d = doy - Math.floor((153 * mp + 2) / 5) + 1;
    var m = mp < 10 ? mp + 3 : mp - 9;
    return { y: m <= 2 ? y + 1 : y, m: m, d: d };
  }

  function dayFromCivil(y, m, d) {
    y -= m <= 2 ? 1 : 0;
    var era = Math.floor(y / 400);
    var yoe = y - era * 400;
    var doy = Math.floor((153 * (m > 2 ? m - 3 : m + 9) + 2) / 5) + d - 1;
    var doe = yoe * 365 + Math.floor(yoe / 4) - Math.floor(yoe / 100) + doy;
    return era * 146097 + doe - 719468;
  }

  // 1970-01-01 was a Thursday. 0 = Sunday … 6 = Saturday.
  function weekdaySun(day) {
    return (((day + 4) % 7) + 7) % 7;
  }

  // Monday-based weekday: 0 = Monday … 6 = Sunday (cell row).
  function weekdayMon(day) {
    return (weekdaySun(day) + 6) % 7;
  }

  // strftime %W: Monday-based week of the year, days before the first
  // Monday are week 00.
  function weekNumberW(day) {
    var c = civilFromDay(day);
    var yday = day - dayFromCivil(c.y, 1, 1); // 0-based day of year
    return Math.floor((yday + 7 - weekdayMon(day)) / 7);
  }

  function daysInMonth(y, m) {
    return dayFromCivil(m === 12 ? y + 1 : y, m === 12 ? 1 : m + 1, 1) -
      dayFromCivil(y, m, 1);
  }

  // ---- English date labels ----

  // "%A %B %-e, %Y" → "Monday July 6, 2026"
  function formatLongDate(day) {
    var c = civilFromDay(day);
    return WEEKDAYS[weekdaySun(day)] + " " + MONTHS[c.m - 1] + " " + c.d +
      ", " + c.y;
  }

  // ---- domain keys: year number, or year*12 + month0 in months mode ----

  function keyOfDay(day) {
    var c = civilFromDay(day);
    return usesYear() ? c.y : c.y * 12 + (c.m - 1);
  }

  function keyStartDay(key) {
    return usesYear()
      ? dayFromCivil(key, 1, 1)
      : dayFromCivil(Math.floor(key / 12), (key % 12) + 1, 1);
  }

  function keyEndDay(key) { // inclusive
    return usesYear()
      ? dayFromCivil(key, 12, 31)
      : keyStartDay(key) +
        daysInMonth(Math.floor(key / 12), (key % 12) + 1) - 1;
  }

  function keyLabel(key) {
    if (usesYear()) return String(key);
    var y = Math.floor(key / 12);
    var m = key % 12;
    return MONTHS[m].slice(0, 3) + " '" + String(y % 100).padStart(2, "0");
  }

  // Columns in a domain block: %W(last) - %W(first) + 1. In year mode the
  // calendar uses the absolute %W as the cell column (including its
  // Jan-1-on-Monday quirk), in month mode the column is relative to the
  // month's first week.
  function keyColumns(key) {
    return weekNumberW(keyEndDay(key)) - weekNumberW(keyStartDay(key)) + 1;
  }

  function cellColumn(key, day) {
    return usesYear()
      ? weekNumberW(day)
      : weekNumberW(day) - weekNumberW(keyStartDay(key));
  }

  function blockWidth(key) { // w(d, true)
    return keyColumns(key) * PITCH + 4;
  }

  // ---- state ----

  var data = {}; // dayIndex -> count (positive past, negative future)
  Object.keys(D.days).forEach(function (k) {
    data[+k] = D.days[k];
  });

  var minDay = D.minDay;
  var dataMaxDay = D.maxDay;
  var maxDay = dataMaxDay;
  var range = visibleRange();
  var initialFirst, first;
  var highlights = [D.today];
  var minReached = false;
  var maxReached = false;
  var animating = false;

  function allowedMinFirst() {
    return keyOfDay(minDay);
  }

  function allowedMaxFirst() {
    return Math.max(allowedMinFirst(), keyOfDay(maxDay) - range + 1);
  }

  function clampFirst(key) {
    return Math.max(allowedMinFirst(), Math.min(allowedMaxFirst(), key));
  }

  function updateReached() {
    minReached = first <= allowedMinFirst();
    maxReached = first >= allowedMaxFirst();
  }

  function initWindow() {
    range = visibleRange();
    maxDay = dataMaxDay;
    if (usesYear()) {
      initialFirst = keyOfDay(D.today);
    } else {
      // Centered month view: today minus 4 months (clamped to the data
      // start); the upper bound is padded to today plus 6 months so the
      // home button can always return to the centered view.
      var todayKey = keyOfDay(D.today);
      initialFirst = Math.max(keyOfDay(minDay), todayKey - Math.floor(range / 2));
      var upperStart = keyStartDay(todayKey + 6);
      if (upperStart > maxDay) maxDay = upperStart;
    }
    first = clampFirst(initialFirst);
  }

  // ---- rendering ----

  function el(tag, attrs, parent) {
    var node = document.createElementNS(SVG_NS, tag);
    for (var k in attrs) node.setAttribute(k, attrs[k]);
    if (parent) parent.appendChild(node);
    return node;
  }

  function cellClasses(day, value) {
    var cls = ["graph-rect"];
    var isHighlight = highlights.indexOf(day) !== -1;
    if (isHighlight) cls.push(day === D.today ? "highlight-now" : "highlight");
    if (day === D.today) cls.push("now");
    else if (day > D.today) cls.push("future");
    if (value !== null) cls.push(qClass(value));
    cls.push("hover_cursor");
    return cls.join(" ");
  }

  function qClass(v) {
    var i;
    for (i = 0; i < D.legend.length; i++) {
      if (v <= D.legend[i]) break;
    }
    var cls = "q" + (i + 1);
    if (v === 0) cls += " q0";
    return cls;
  }

  function renderDomain(key, x) {
    var cols = keyColumns(key);
    var domain = el("g", {
      "class": "graph-domain",
      transform: "translate(" + x + ",0)",
    });
    el("rect", {
      "class": "domain-background",
      width: cols * PITCH,
      height: BLOCK_H - 4,
    }, domain);
    var group = el("g", {
      "class": "graph-subdomain-group",
      transform: "translate(1,1)",
    }, domain);
    var start = keyStartDay(key);
    var end = keyEndDay(key);
    for (var day = start; day <= end; day++) {
      var value = Object.prototype.hasOwnProperty.call(data, day)
        ? data[day]
        : null;
      var g = el("g", {}, group);
      el("rect", {
        "class": cellClasses(day, value),
        width: CELL,
        height: CELL,
        x: cellColumn(key, day) * PITCH,
        y: weekdayMon(day) * PITCH,
        "data-day": day,
        role: "button",
        tabindex: day === D.today ? 0 : -1,
        "aria-label": formatLongDate(day) + ". " + (value === null ? "No activity" : Math.abs(value) + (value < 0 ? " cards due" : " reviews")),
      }, g);
    }
    var label = el("text", {
      "class": "graph-label",
      x: 1 + (cols * PITCH) / 2,
      y: 1 + 7 * PITCH + LABEL_H / 2,
      "text-anchor": "middle",
      "dominant-baseline": "middle",
    }, domain);
    label.textContent = keyLabel(key);
    return domain;
  }

  // Renders the visible window (optionally as a wider strip for the slide
  // animation) and returns layout info.
  function renderStrip(firstKey, extraBefore, extraAfter) {
    var cal = root.querySelector("#rh-cal");
    cal.textContent = "";
    var dayLabelWidth = !usesYear() ? PITCH : 0;

    var stripFirst = firstKey - extraBefore;
    var keys = [];
    for (var k = stripFirst; k < firstKey + range + extraAfter; k++) {
      keys.push(k);
    }
    var xs = {};
    var x = 0;
    keys.forEach(function (key) {
      xs[key] = x;
      x += blockWidth(key);
    });

    var windowWidth = 0;
    for (var i = 0; i < range; i++) windowWidth += blockWidth(firstKey + i);

    var svg = el("svg", {
      "class": "cal-heatmap-container",
      width: windowWidth - 4 + dayLabelWidth,
      height: SVG_H,
    });
    var graph = el("g", { "class": "graph" }, svg);
    keys.forEach(function (key) {
      graph.appendChild(renderDomain(key, xs[key]));
    });
    if (!usesYear()) {
      var dayNames = el("g", { "class": "day-name" }, svg);
      var letters = ["M", "T", "W", "T", "F", "S", "S"];
      letters.forEach(function (letter, index) {
        el("rect", {
          "class": "day-name-rect",
          width: CELL,
          height: CELL,
          x: 0,
          y: index * PITCH,
        }, dayNames);
        var text = el("text", {
          "class": "day-name-text",
          x: 0,
          y: index * PITCH + 5,
          "dominant-baseline": "central",
        }, dayNames);
        text.textContent = letter;
      });
    }
    cal.appendChild(svg);
    return {
      svg: svg,
      graph: graph,
      xs: xs,
      dayLabelWidth: dayLabelWidth,
      originX: xs[firstKey],
    };
  }

  var layout = null;
  var currentShift = 0;

  function setGraphShift(shift) {
    currentShift = shift;
    layout.graph.setAttribute(
      "transform",
      "translate(" + (layout.dayLabelWidth - shift) + ",0)",
    );
  }

  function render() {
    root.classList.toggle("rh-mobile", mobile);
    root.setAttribute("data-layout", mobile ? "fragment" : "overview");
    layout = renderStrip(first, 0, 0);
    setGraphShift(layout.originX);
    updateReached();
    updateToolbar();
    reportHeight();
  }

  // Animate navigation with a 500 ms horizontal slide. Move the whole strip
  // between window offsets so every surviving block shifts equally.
  function slideTo(newFirst) {
    if (animating || newFirst === first) return;
    hideTooltip();
    if (mobile || window.matchMedia("(prefers-reduced-motion: reduce)").matches) {
      first = newFirst;
      render();
      return;
    }
    var oldFirst = first;
    first = newFirst;
    var before = Math.max(0, oldFirst - newFirst);
    var after = Math.max(0, newFirst - oldFirst);
    layout = renderStrip(Math.min(oldFirst, newFirst), 0, before + after);
    var fromShift = layout.xs[oldFirst];
    var toShift = layout.xs[newFirst];
    animating = true;
    var startTime = null;
    var DURATION = 500;
    function ease(t) { // cubic in-out, d3's default
      t *= 2;
      return (t <= 1 ? t * t * t : (t -= 2) * t * t + 2) / 2;
    }
    function step(now) {
      if (startTime === null) startTime = now;
      var t = Math.min(1, (now - startTime) / DURATION);
      setGraphShift(fromShift + (toShift - fromShift) * ease(t));
      if (t < 1) {
        requestAnimationFrame(step);
      } else {
        animating = false;
        render();
      }
    }
    setGraphShift(fromShift);
    requestAnimationFrame(step);
    updateReached();
  }

  function updateToolbar() {
    var label = root.querySelector("#rh-period-label");
    if (!label) return;
    label.textContent = keyLabel(first) + (range > 1 ? " – " + keyLabel(first + range - 1) : "");
    root.querySelector("#rh-btn-prev").disabled = minReached;
    root.querySelector("#rh-btn-next").disabled = maxReached;
  }

  function updateResponsiveLayout() {
    var next = isMobileLayout(), width = root.clientWidth;
    if (next !== mobile || (mobile && width !== viewportWidth)) {
      mobile = next;
      viewportWidth = width;
      hideTooltip();
      if (root.querySelector("#rh-cal")) {
        initWindow();
        render();
      }
    }
    reportHeight();
  }

  // ---- tooltip ----

  var tooltip = null;

  function tooltipHtml(day, value) {
    var date = formatLongDate(day);
    if (value === null) {
      return day > D.today
        ? "<b>No</b> cards due on " + date
        : "<b>No</b> reviews on " + date;
    }
    var count = Math.abs(value);
    var name = count === 1 ? "card" : "cards";
    var verb = value < 0 ? "due" : "reviewed";
    return "<b>" + count + "</b> " + name + " <b>" + verb + "</b> on " + date;
  }

  function showTooltip(rect, day, value) {
    var cal = root.querySelector("#rh-cal");
    if (!tooltip) {
      tooltip = document.createElement("div");
      tooltip.className = "ch-tooltip";
    }
    if (tooltip.parentNode !== cal) {
      // Re-renders replace #rh-cal's children; re-attach the tooltip.
      cal.appendChild(tooltip);
    }
    tooltip.innerHTML = tooltipHtml(day, value);
    tooltip.style.display = "block";
    // Use the painted cell position, including SVG navigation transforms.
    // A high z-index cannot escape the embedded browser's viewport.
    var cell = rect.getBoundingClientRect();
    var gap = 8;
    var margin = 4;
    var width = tooltip.offsetWidth;
    var height = tooltip.offsetHeight;
    var center = cell.left + cell.width / 2;
    var x = Math.max(margin, Math.min(center - width / 2,
      window.innerWidth - width - margin));
    var above = cell.top - height - gap;
    var below = above < margin;
    var y = below ? cell.bottom + gap : above;
    y = Math.max(margin, Math.min(y, window.innerHeight - height - margin));
    tooltip.classList.toggle("ch-tooltip-below", below);
    tooltip.style.setProperty("--rh-arrow-x",
      Math.max(10, Math.min(width - 10, center - x)) + "px");
    tooltip.style.left = x + "px";
    tooltip.style.top = y + "px";
  }

  function hideTooltip() {
    if (tooltip) {
      tooltip.style.display = "none";
      tooltip.innerHTML = "";
    }
  }

  // ---- interactions ----

  function bridgeSend(action) {
    try {
      var channel = window.srsExtensions;
      if (channel && typeof channel.postMessage === "function") {
        channel.postMessage(JSON.stringify(action));
        return true;
      }
    } catch (e) { /* host without a bridge */ }
    return false;
  }

  function saveConfig() {
    try {
      fetch("/api/v1/srs/extensions/review-heatmap/config", {
        method: "PUT",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(D.config),
      });
    } catch (e) { /* config persistence is best-effort */ }
  }

  function setHighlight(days) {
    highlights = days;
    render();
  }

  function onCellClick(day, value) {
    if (value === null || value === 0) {
      setHighlight([D.today]);
      return;
    }
    var cmd;
    if (value >= 0) {
      // The whole local day as a unix-ms review-time window.
      var start = (day * 86400 - D.offset) * 1000;
      cmd = "rid:" + start + ":" + (start + 86400000);
    } else {
      cmd = "prop:due=" + (day - D.today);
    }
    bridgeSend({ type: "openBrowser", query: cmd });
    setHighlight([D.today, day]);
  }

  function onNavigate(event, direction) {
    if (direction === "prev") {
      if (event.shiftKey) {
        // Jump to the first page — a no-op if it is already visible.
        var minKey = keyOfDay(minDay);
        if (minKey < first) slideTo(clampFirst(minKey));
      } else if (!minReached) {
        slideTo(clampFirst(first - range));
      }
    } else {
      if (event.shiftKey) {
        var maxKey = keyOfDay(maxDay);
        if (maxKey > first + range - 1) {
          slideTo(clampFirst(maxKey - range + 1));
        }
      } else if (!maxReached) {
        slideTo(clampFirst(first + range));
      }
    }
  }

  function onHome(event) {
    if (event.shiftKey && !mobile) {
      D.mode = MODES[(MODES.indexOf(D.mode) + 1) % MODES.length];
      D.config.mode = D.mode;
      saveConfig();
      root.classList.remove("rh-mode-year", "rh-mode-months");
      root.classList.add("rh-mode-" + D.mode);
      highlights = [D.today];
      initWindow();
      render();
    } else {
      slideTo(clampFirst(initialFirst));
    }
  }

  function openOptions() {
    // Ask the extension (over the generic on-message bridge) to open its
    // options dialog via ui.open-dialog.
    bridgeSend({
      type: "extensionMessage",
      extensionId: "review-heatmap",
      payload: JSON.stringify({ type: "openOptions" }),
    });
  }

  function onOptions(event) {
    if (event.shiftKey) {
      var theme = THEMES[(THEMES.indexOf(D.theme) + 1) % THEMES.length];
      root.classList.remove("rh-theme-" + D.theme);
      root.classList.add("rh-theme-" + theme);
      D.theme = theme;
      D.config.colors = theme;
      saveConfig();
    } else {
      openOptions();
    }
  }

  function reportHeight() {
    bridgeSend({
      type: "resize",
      // Include the top margin and any preceding dashboard content.
      height: Math.ceil(root.getBoundingClientRect().bottom + window.scrollY) + 2,
    });
  }

  function wire() {
    root.addEventListener("pointerdown", function (event) {
      if (!event.target.closest("#rh-cal")) hideTooltip();
    }, { passive: true });
    var cal = root.querySelector("#rh-cal");
    if (cal) {
      cal.addEventListener("scroll", hideTooltip, { passive: true });
      var swipe = null;
      cal.addEventListener("pointerdown", function (event) {
        hideTooltip();
        suppressSwipeClick = false;
        if (mobile && event.isPrimary && event.pointerType !== "mouse") swipe = { x: event.clientX, y: event.clientY };
      });
      cal.addEventListener("pointercancel", function () { swipe = null; });
      cal.addEventListener("pointerup", function (event) {
        if (!swipe || !mobile) return;
        var dx = event.clientX - swipe.x, dy = event.clientY - swipe.y;
        swipe = null;
        if (Math.abs(dx) >= 48 && Math.abs(dx) > Math.abs(dy) * 1.4) {
          suppressSwipeClick = true;
          onNavigate({ shiftKey: false }, dx > 0 ? "prev" : "next");
        }
      });
      cal.addEventListener("keydown", function (event) {
        var rect = event.target.closest("[data-day]");
        if (!rect || (event.key !== "Enter" && event.key !== " ")) return;
        event.preventDefault();
        var day = +rect.dataset.day;
        onCellClick(day, Object.prototype.hasOwnProperty.call(data, day) ? data[day] : null);
      });
      cal.addEventListener("click", function (event) {
        var rect = event.target.closest ? event.target.closest("rect[data-day]") : null;
        if (!rect) return;
        if (suppressSwipeClick) { suppressSwipeClick = false; return; }
        var day = +rect.getAttribute("data-day");
        var value = Object.prototype.hasOwnProperty.call(data, day) ? data[day] : null;
        if (mobile && !value) showTooltip(rect, day, value);
        else onCellClick(day, value);
      });
      cal.addEventListener("mouseover", function (event) {
        if (mobile) return;
        var rect = event.target.closest ? event.target.closest("rect[data-day]") : null;
        if (!rect) return;
        var day = +rect.getAttribute("data-day");
        showTooltip(rect, day, Object.prototype.hasOwnProperty.call(data, day) ? data[day] : null);
      });
      cal.addEventListener("mouseout", function (event) {
        var rect = event.target.closest ? event.target.closest("rect[data-day]") : null;
        if (rect) hideTooltip();
      });
    }
    var prev = root.querySelector("#rh-btn-prev");
    var home = root.querySelector("#rh-btn-home");
    var next = root.querySelector("#rh-btn-next");
    var opts = root.querySelector("#rh-btn-opts");
    if (prev) prev.addEventListener("click", function (e) { onNavigate(e, "prev"); });
    if (next) next.addEventListener("click", function (e) { onNavigate(e, "next"); });
    if (home) home.addEventListener("click", onHome);
    if (opts) opts.addEventListener("click", onOptions);
    var noDataOpts = root.querySelector("#rh-nodata-opts");
    if (noDataOpts) {
      noDataOpts.addEventListener("click", openOptions);
    }
  }

  // The native host can attach its bridge after the document first renders.
  window.addEventListener("manatan-srs-host-ready", reportHeight);
  window.addEventListener("resize", hideTooltip);
  new ResizeObserver(updateResponsiveLayout).observe(root);

  wire();
  if (root.querySelector("#rh-cal")) {
    initWindow();
    render();
  } else {
    reportHeight();
  }
})();
