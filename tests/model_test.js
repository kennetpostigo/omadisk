#!/usr/bin/env node
"use strict";

const fs = require("fs");
const path = require("path");

const root = path.resolve(__dirname, "..");
const src = fs
  .readFileSync(path.join(root, "bar/Model.js"), "utf8")
  .replace(/^\s*\.pragma library\s*/m, "");
const Model = {};
new Function(
  "exports",
  src +
    "\n" +
    ["asPlainLabel", "chipTooltip", "bytes", "chipIcon"].map(
      (k) => `exports.${k} = typeof ${k} === "function" ? ${k} : undefined;`
    ).join("\n")
)(Model);

let failed = 0;
function assert(cond, msg) {
  if (!cond) {
    failed += 1;
    console.error("FAIL", msg);
  }
}

const img = '/home/x/<img src="https://evil.example/x.png">';
const md = "/home/x/![hit](https://evil.example/x.png)";
assert(Model.asPlainLabel(img).indexOf("<") === -1, "HTML tags are neutralized");
assert(Model.asPlainLabel(img).indexOf(">") === -1, "HTML closers are neutralized");
assert(Model.asPlainLabel(md).indexOf("![") === -1, "markdown image marker is broken");
assert(Model.asPlainLabel("/home/postman").indexOf("/home/postman") === 0, "ordinary paths stay readable");

const tip = Model.chipTooltip({ ok: true, free: 1024 }, img);
assert(tip.indexOf("<img") === -1, "chip tooltip does not keep an img tag");
assert(tip.indexOf("click to open Omadisk") !== -1, "chip tooltip keeps the action hint");

if (failed) {
  console.error(failed + " bar model assertion(s) failed");
  process.exit(1);
}
console.log("bar model ok");
