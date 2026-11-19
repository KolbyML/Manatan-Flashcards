/* Review Heatmap options dialog (runs inside a `ui.open-dialog` page).
 *
 * The host renders this HTML in a dialog webview and exposes the same
 * `srsExtensions` bridge as dashboard surfaces. Saving posts a
 * `saveOptions` message back into the extension's `on-message` hook, which
 * persists the config and asks the host to refresh the dashboard surface.
 */
(function () {
  "use strict";

  var config = window.RH_OPTS || {};

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

  function byId(id) {
    return document.getElementById(id);
  }

  function fill(values) {
    byId("rh-opt-mode").value = values.mode || "year";
    byId("rh-opt-limhist").value = values.limhist || 0;
    byId("rh-opt-limfcst").value = values.limfcst || 0;
    var limdate = "";
    if (values.limdate > 0) {
      var date = new Date(values.limdate);
      limdate = date.getFullYear() + "-" +
        String(date.getMonth() + 1).padStart(2, "0") + "-" +
        String(date.getDate()).padStart(2, "0");
    }
    byId("rh-opt-limdate").value = limdate;
    var display = values.display || {};
    byId("rh-opt-deckbrowser").checked = display.deckbrowser !== false;
    byId("rh-opt-stats").checked = display.stats !== false;
    byId("rh-opt-statsvis").checked = values.statsvis !== false;
  }

  function collect() {
    var limdateMs = 0;
    var limdate = byId("rh-opt-limdate").value;
    if (limdate) {
      var parts = limdate.split("-");
      limdateMs = new Date(+parts[0], +parts[1] - 1, +parts[2]).getTime();
    }
    return {
      // Keep existing collections compatible while the app owns appearance.
      colors: config.colors || "lime",
      mode: byId("rh-opt-mode").value,
      limhist: Math.max(0, parseInt(byId("rh-opt-limhist").value, 10) || 0),
      limfcst: Math.max(0, parseInt(byId("rh-opt-limfcst").value, 10) || 0),
      limdate: limdateMs,
      display: {
        deckbrowser: byId("rh-opt-deckbrowser").checked,
        stats: byId("rh-opt-stats").checked,
      },
      statsvis: byId("rh-opt-statsvis").checked,
    };
  }

  byId("rh-opt-defaults").addEventListener("click", function () {
    fill({
      mode: "year",
      limhist: 0,
      limfcst: 0,
      limdate: 0,
      display: { deckbrowser: true, stats: true },
      statsvis: true,
    });
  });

  byId("rh-opt-cancel").addEventListener("click", function () {
    bridgeSend({ type: "closeDialog" });
  });

  byId("rh-opt-ok").addEventListener("click", function () {
    bridgeSend({
      type: "extensionMessage",
      extensionId: "review-heatmap",
      payload: JSON.stringify({ type: "saveOptions", config: collect() }),
    });
    bridgeSend({ type: "closeDialog" });
  });

  fill(config);
  bridgeSend({
    type: "resize",
    height: Math.ceil(document.body.scrollHeight) + 8,
  });
})();
