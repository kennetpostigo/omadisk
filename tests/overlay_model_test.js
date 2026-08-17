#!/usr/bin/env node
"use strict";

const fs = require("fs");
const path = require("path");

const root = path.resolve(__dirname, "..");
const src = fs
  .readFileSync(path.join(root, "overlay/OverlayModel.js"), "utf8")
  .replace(/^\s*\.pragma library\s*/m, "");
const Model = {};
new Function("exports", src + "\n" + Object.getOwnPropertyNames({
  otherPath: 0, isOtherPath: 0, parseLine: 0, countSliceNodes: 0,
  folderFill: 0, layoutSlices: 0, findNode: 0, project: 0,
  hasSlicePath: 0, fillForPath: 0, sliceByPath: 0, hitTestSlices: 0,
  angleContains: 0, parentPath: 0, normalizePath: 0, basename: 0,
}).map((k) => `exports.${k} = typeof ${k} === "function" ? ${k} : undefined;`).join("\n"))(Model);

const GEOM = {
  sliceGapDeg: 0.6,
  minSweepDeg: 2.0,
  hubRadius: 40,
  ringWidth: 22,
  ringPad: 2,
};

let failed = 0;
function assert(cond, msg) {
  if (!cond) {
    failed += 1;
    console.error("FAIL", msg);
  }
}

function sliceCentroid(slice, cx, cy) {
  const midDeg = slice.startDeg + slice.sweepDeg / 2;
  const rad = (midDeg * Math.PI) / 180;
  const midR = (slice.innerR + slice.outerR) / 2;
  return { x: cx + midR * Math.cos(rad), y: cy + midR * Math.sin(rad) };
}

function ringOverlaps(slices, ring) {
  const onRing = slices
    .filter((s) => s.ring === ring)
    .map((s) => {
      const start = ((s.startDeg % 360) + 360) % 360;
      return { path: s.path, start, end: start + s.sweepDeg };
    })
    .sort((a, b) => a.start - b.start);
  for (let i = 1; i < onRing.length; i++) {
    if (onRing[i].start + 0.05 < onRing[i - 1].end) {
      return [onRing[i - 1], onRing[i]];
    }
  }
  if (onRing.length > 1) {
    const last = onRing[onRing.length - 1];
    const first = onRing[0];
    if (last.end > 360 && last.end - 360 > first.start + 0.05) {
      return [last, first];
    }
  }
  return null;
}

const golden = JSON.parse(fs.readFileSync(path.join(root, "tests/goldens/view-depth3.json"), "utf8"));
const goldenSlices = Model.layoutSlices(golden, -90, GEOM);
assert(goldenSlices.length >= 2, "golden view should produce slices");
const cacheSlice = Model.sliceByPath(goldenSlices, "/home/postman/.cache");
assert(cacheSlice && cacheSlice.ring === 1, "golden .cache is ring 1");
assert(cacheSlice.sweepDeg > 300, "golden .cache dominates the ring (got " + (cacheSlice && cacheSlice.sweepDeg) + ")");
assert(Model.hasSlicePath(goldenSlices, "/home/postman/\0other"), "golden Other slice exists");
assert(!Model.hasSlicePath(goldenSlices, "/home/postman/notes.txt"), "tiny list row is not a slice");
assert(Model.fillForPath(goldenSlices, "/home/postman/notes.txt") === "", "collapsed list row does not inherit Other fill");
assert(Model.fillForPath(goldenSlices, "/home/postman/\0other") !== "", "Other has its own fill");

const hitCache = sliceCentroid(cacheSlice, 100, 100);
const hit = Model.hitTestSlices(hitCache.x, hitCache.y, 100, 100, goldenSlices, GEOM.hubRadius);
assert(hit.kind === "slice" && hit.slice.path === cacheSlice.path, "hit-test at .cache centroid returns .cache");

const homeView = {
  v: 1,
  type: "view",
  path: "/home/postman",
  name: "postman",
  bytes: 68479504384,
  children: [
    {
      name: ".local",
      path: "/home/postman/.local",
      kind: "dir",
      bytes: 63604244480,
      children: [
        { name: "share", path: "/home/postman/.local/share", kind: "dir", bytes: 63000000000, children: [] },
        { name: "state", path: "/home/postman/.local/state", kind: "dir", bytes: 16000000, children: [] },
      ],
    },
    {
      name: ".cache",
      path: "/home/postman/.cache",
      kind: "dir",
      bytes: 2042036224,
      children: [],
    },
    {
      name: ".rustup",
      path: "/home/postman/.rustup",
      kind: "dir",
      bytes: 1655504896,
      children: [],
    },
    {
      name: "Other",
      path: "/home/postman/\0other",
      kind: "other",
      bytes: 1177718784,
      children: [],
    },
  ],
  list: [
    { name: ".local", path: "/home/postman/.local", kind: "dir", bytes: 63604244480 },
    { name: ".cache", path: "/home/postman/.cache", kind: "dir", bytes: 2042036224 },
    { name: "Pictures", path: "/home/postman/Pictures", kind: "dir", bytes: 1974272 },
    { name: "Documents", path: "/home/postman/Documents", kind: "dir", bytes: 0 },
  ],
};

const slices = Model.layoutSlices(homeView, -90, GEOM);
const local = Model.sliceByPath(slices, "/home/postman/.local");
const other = Model.sliceByPath(slices, "/home/postman/\0other");
assert(local, ".local has a slice");
assert(other && other.kind === "other", "Other has a slice");
assert(local.sweepDeg > 300, ".local sweep should be ~93% of 360 (got " + local.sweepDeg + ")");
assert(other.sweepDeg > 2 && other.sweepDeg < 20, "Other is a small distinct wedge (got " + other.sweepDeg + ")");

assert(!Model.hasSlicePath(slices, "/home/postman/Pictures"), "Pictures is collapsed, no own slice");
assert(!Model.hasSlicePath(slices, "/home/postman/Documents"), "Documents is collapsed, no own slice");
assert(Model.fillForPath(slices, "/home/postman/Pictures") === "", "Pictures does not share Other fill");
assert(Model.fillForPath(slices, "/home/postman/Documents") === "", "Documents does not share Other fill");
assert(
  Model.sliceByPath(slices, "/home/postman/Pictures") === null
    && Model.sliceByPath(slices, "/home/postman/Documents") === null,
  "Pictures and Documents do not resolve to any slice, including each other"
);

const paths = slices.map((s) => s.ring + ":" + s.path);
assert(new Set(paths).size === paths.length, "slice paths are unique per ring");
assert(!ringOverlaps(slices, 1), "ring 1 slices do not overlap");
assert(!ringOverlaps(slices, 2), "ring 2 slices do not overlap");

const hubHit = Model.hitTestSlices(100, 100, 100, 100, slices, GEOM.hubRadius);
assert(hubHit.kind === "hub", "center is the hub");

const otherHitPt = sliceCentroid(other, 100, 100);
const otherHit = Model.hitTestSlices(otherHitPt.x, otherHitPt.y, 100, 100, slices, GEOM.hubRadius);
assert(otherHit.kind === "slice" && otherHit.slice.path === other.path, "Other centroid hits Other, not Pictures/Documents");
assert(otherHit.slice.path !== "/home/postman/Pictures", "Other hit is not Pictures");
assert(otherHit.slice.path !== "/home/postman/Documents", "Other hit is not Documents");

const localHitPt = sliceCentroid(local, 100, 100);
const localHit = Model.hitTestSlices(localHitPt.x, localHitPt.y, 100, 100, slices, GEOM.hubRadius);
assert(localHit.kind === "slice" && localHit.slice.path === local.path, " .local centroid hits .local");

if (failed) {
  console.error(failed + " overlay model assertion(s) failed");
  process.exit(1);
}
console.log("overlay model ok (" + slices.length + " home slices, .local sweep " + local.sweepDeg.toFixed(1) + "deg)");
