/* Apply the host-selected palette before rendering the extension UI. */
(function () {
  "use strict";
  var encoded = new URLSearchParams(window.location.hash.slice(1)).get("manatanTheme");
  if (!encoded) return;
  try {
    var palette = JSON.parse(encoded);
    ["foreground", "muted", "surface", "border", "primary", "error"].forEach(function (name) {
      var color = palette[name];
      if (typeof color === "string" && /^#[0-9a-f]{6}$/i.test(color)) {
        document.documentElement.style.setProperty("--manatan-" + name, color);
      }
    });
  } catch (_) { /* Existing hosts use the extension's fallback colors. */ }
})();
