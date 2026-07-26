import assert from "node:assert/strict";
import { test } from "node:test";

import { APP_NAME, PORTABLE_MARKER, portableArchiveName, portableReadme } from "./make-portable.mjs";

test("portableArchiveName matches the name the updater manifest points at", () => {
  assert.equal(portableArchiveName("1.2.3"), "LocalMind_1.2.3_x64-portable.zip");
  assert.equal(portableArchiveName("0.1.0"), "LocalMind_0.1.0_x64-portable.zip");
});

test("portableArchiveName rejects a version that is not semantic", () => {
  assert.throws(() => portableArchiveName("v1.2.3"), /invalid version/);
  assert.throws(() => portableArchiveName("1.2"), /invalid version/);
  assert.throws(() => portableArchiveName(undefined), /invalid version/);
});

test("the marker name is the one the Rust side looks for", () => {
  // update::flavor() checks for this exact file next to the executable.
  assert.equal(PORTABLE_MARKER, ".portable");
  assert.equal(APP_NAME, "LocalMind");
});

test("portableReadme tells the user where the data lives and not to delete the marker", () => {
  const readme = portableReadme("1.2.3");
  assert.match(readme, /LocalMind 1\.2\.3/);
  assert.match(readme, /\.\/data/);
  assert.match(readme, /administrador/);
  assert.ok(readme.includes(PORTABLE_MARKER));
});
