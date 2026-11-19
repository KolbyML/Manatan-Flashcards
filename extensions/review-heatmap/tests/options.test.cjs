const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const test = require("node:test");
const vm = require("node:vm");

const sourceRoot = path.join(__dirname, "..", "src");
const optionsScript = fs.readFileSync(path.join(sourceRoot, "web", "options.js"), "utf8");
const rust = fs.readFileSync(path.join(sourceRoot, "lib.rs"), "utf8");
const dialog = rust.slice(rust.indexOf('fn options_dialog_html'), rust.indexOf('impl Extension for ReviewHeatmap'));
const original = {
  colors: "ice",
  mode: "months",
  limhist: 90,
  limfcst: 30,
  limdate: new Date(2025, 2, 9).getTime(),
  display: { deckbrowser: false, stats: true },
  statsvis: false,
};

function openOptions(config = original) {
  const fields = new Map();
  for (const match of dialog.matchAll(/id="(rh-opt-[^"]+)"/g)) {
    const handlers = {};
    fields.set(match[1], {
      value: "",
      checked: false,
      addEventListener(type, listener) { handlers[type] = listener; },
      click() { handlers.click(); },
    });
  }
  const messages = [];
  vm.runInNewContext(optionsScript, {
    window: {
      RH_OPTS: structuredClone(config),
      srsExtensions: { postMessage: (message) => messages.push(JSON.parse(message)) },
    },
    document: {
      getElementById: (id) => fields.get(id) ?? null,
      body: { scrollHeight: 320 },
    },
    Date,
  });
  return {
    fields,
    messages,
    save() {
      fields.get("rh-opt-ok").click();
      const message = messages.find((item) => item.type === "extensionMessage");
      return JSON.parse(message.payload).config;
    },
  };
}

test("options omit Color scheme and load every remaining saved setting", () => {
  const page = openOptions();
  assert.equal(page.fields.has("rh-opt-colors"), false);
  assert.doesNotMatch(dialog, />Color scheme</);
  assert.equal(page.fields.get("rh-opt-mode").value, "months");
  assert.equal(page.fields.get("rh-opt-limhist").value, 90);
  assert.equal(page.fields.get("rh-opt-limfcst").value, 30);
  assert.equal(page.fields.get("rh-opt-limdate").value, "2025-03-09");
  assert.equal(page.fields.get("rh-opt-deckbrowser").checked, false);
  assert.equal(page.fields.get("rh-opt-stats").checked, true);
  assert.equal(page.fields.get("rh-opt-statsvis").checked, false);
  assert.deepEqual(page.messages, [{ type: "resize", height: 328 }]);
});

test("OK saves all remaining options and preserves the hidden legacy palette", () => {
  const page = openOptions();
  page.fields.get("rh-opt-mode").value = "year";
  page.fields.get("rh-opt-limhist").value = "120";
  page.fields.get("rh-opt-limfcst").value = "60";
  page.fields.get("rh-opt-limdate").value = "2026-11-01";
  page.fields.get("rh-opt-deckbrowser").checked = true;
  page.fields.get("rh-opt-stats").checked = false;
  page.fields.get("rh-opt-statsvis").checked = true;
  assert.deepEqual(page.save(), {
    colors: "ice", mode: "year", limhist: 120, limfcst: 60,
    limdate: new Date(2026, 10, 1).getTime(),
    display: { deckbrowser: true, stats: false }, statsvis: true,
  });
  assert.equal(page.messages.at(-1).type, "closeDialog");
});

test("Restore Defaults resets visible options and Cancel leaves saved options intact", () => {
  const page = openOptions();
  page.fields.get("rh-opt-defaults").click();
  assert.equal(page.fields.get("rh-opt-mode").value, "year");
  assert.equal(page.fields.get("rh-opt-limhist").value, 0);
  assert.equal(page.fields.get("rh-opt-limfcst").value, 0);
  assert.equal(page.fields.get("rh-opt-limdate").value, "");
  assert.equal(page.fields.get("rh-opt-deckbrowser").checked, true);
  assert.equal(page.fields.get("rh-opt-stats").checked, true);
  assert.equal(page.fields.get("rh-opt-statsvis").checked, true);
  assert.equal(page.messages.filter((item) => item.type === "extensionMessage").length, 0);
  page.fields.get("rh-opt-cancel").click();
  assert.equal(page.messages.at(-1).type, "closeDialog");
  assert.equal(page.messages.filter((item) => item.type === "extensionMessage").length, 0);
  const defaults = openOptions();
  defaults.fields.get("rh-opt-defaults").click();
  assert.deepEqual(defaults.save(), {
    colors: "ice", mode: "year", limhist: 0, limfcst: 0, limdate: 0,
    display: { deckbrowser: true, stats: true }, statsvis: true,
  });
});

test("empty configuration, blank dates, zero limits, and negative limits save safely", () => {
  const page = openOptions({});
  assert.deepEqual(page.save(), {
    colors: "lime", mode: "year", limhist: 0, limfcst: 0, limdate: 0,
    display: { deckbrowser: true, stats: true }, statsvis: true,
  });
  const edited = openOptions();
  edited.fields.get("rh-opt-limhist").value = "-30";
  edited.fields.get("rh-opt-limfcst").value = "";
  edited.fields.get("rh-opt-limdate").value = "";
  const saved = edited.save();
  assert.equal(saved.limhist, 0);
  assert.equal(saved.limfcst, 0);
  assert.equal(saved.limdate, 0);
});
