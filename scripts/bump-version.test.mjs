import assert from "node:assert/strict";
import { test } from "node:test";

import {
  bumpVersion,
  parseVersion,
  setCargoVersion,
  setJsonVersion,
  stripTagPrefix,
} from "./bump-version.mjs";

test("parseVersion accepts a well-formed version", () => {
  assert.deepEqual(parseVersion("1.2.3"), { major: 1, minor: 2, patch: 3 });
});

test("parseVersion rejects garbage", () => {
  assert.throws(() => parseVersion("1.2"), /invalid semantic version/);
  assert.throws(() => parseVersion("v1.2.3"), /invalid semantic version/);
  assert.throws(() => parseVersion(undefined), /invalid semantic version/);
});

test("bumpVersion applies each kind", () => {
  assert.equal(bumpVersion("1.2.3", "patch"), "1.2.4");
  assert.equal(bumpVersion("1.2.3", "minor"), "1.3.0");
  assert.equal(bumpVersion("1.2.3", "major"), "2.0.0");
});

test("bumpVersion resets the lower components", () => {
  assert.equal(bumpVersion("0.1.0", "minor"), "0.2.0");
  assert.equal(bumpVersion("1.9.7", "major"), "2.0.0");
  assert.equal(bumpVersion("0.1.9", "patch"), "0.1.10");
});

test("bumpVersion rejects an unknown kind", () => {
  assert.throws(() => bumpVersion("1.0.0", "huge"), /unknown bump kind/);
});

test("stripTagPrefix removes the git tag v", () => {
  assert.equal(stripTagPrefix("v1.2.3"), "1.2.3");
  assert.equal(stripTagPrefix("1.2.3"), "1.2.3");
  assert.equal(stripTagPrefix(""), "");
});

test("setJsonVersion updates the root version", () => {
  const out = JSON.parse(setJsonVersion('{"name":"x","version":"0.1.0"}', "0.2.0"));
  assert.equal(out.version, "0.2.0");
  assert.equal(out.name, "x");
});

test("setJsonVersion also updates the lockfile root package entry", () => {
  const source = JSON.stringify({
    name: "localmind",
    version: "0.1.0",
    packages: { "": { name: "localmind", version: "0.1.0" }, "node_modules/react": { version: "19.1.0" } },
  });
  const out = JSON.parse(setJsonVersion(source, "1.0.0"));
  assert.equal(out.version, "1.0.0");
  assert.equal(out.packages[""].version, "1.0.0");
  assert.equal(out.packages["node_modules/react"].version, "19.1.0", "dependency versions must not move");
});

test("setCargoVersion only touches the [package] version", () => {
  const source = [
    "[package]",
    'name = "tauri-app"',
    'version = "0.1.0"',
    'edition = "2021"',
    "",
    "[dependencies]",
    'serde = { version = "1", features = ["derive"] }',
    'rusqlite = { version = "0.31" }',
    "",
  ].join("\n");

  const out = setCargoVersion(source, "0.2.0");
  assert.match(out, /\[package\][\s\S]*version = "0\.2\.0"/);
  assert.match(out, /serde = \{ version = "1"/, "dependency versions must not move");
  assert.match(out, /rusqlite = \{ version = "0\.31" \}/);
});

test("setCargoVersion fails loudly on an unexpected manifest", () => {
  assert.throws(() => setCargoVersion('[dependencies]\nserde = "1"\n', "1.0.0"), /\[package\] section not found/);
  assert.throws(() => setCargoVersion('[package]\nname = "x"\n', "1.0.0"), /version key not found/);
});
