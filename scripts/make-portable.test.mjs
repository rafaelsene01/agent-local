import assert from "node:assert/strict";
import { test } from "node:test";
import { existsSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import {
  APP_NAME,
  PORTABLE_MARKER,
  RESOURCES_DIR,
  portableArchiveName,
  portableReadme,
  stageBundle,
} from "./make-portable.mjs";

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

test("the staged bundle carries the runtime components next to the executable", () => {
  const root = mkdtempSync(join(tmpdir(), "localmind-portable-"));
  const build = join(root, "release");
  mkdirSync(join(build, RESOURCES_DIR, "llama", "vulkan"), { recursive: true });
  writeFileSync(join(build, `${APP_NAME}.exe`), "binary");
  writeFileSync(join(build, RESOURCES_DIR, "llama", "vulkan", "llama-server.exe"), "server");

  const appDir = join(root, "staging", APP_NAME);
  stageBundle({
    appDir,
    binary: join(build, `${APP_NAME}.exe`),
    resources: join(build, RESOURCES_DIR),
    version: "1.2.3",
  });

  assert.ok(existsSync(join(appDir, `${APP_NAME}.exe`)));
  assert.ok(existsSync(join(appDir, PORTABLE_MARKER)));
  assert.ok(
    existsSync(join(appDir, RESOURCES_DIR, "llama", "vulkan", "llama-server.exe")),
    "a portable bundle without llama-server cannot answer anything",
  );

  rmSync(root, { recursive: true, force: true });
});

test("missing resources fail the packaging instead of producing a mute broken zip", () => {
  const root = mkdtempSync(join(tmpdir(), "localmind-portable-"));
  const build = join(root, "release");
  mkdirSync(build, { recursive: true });
  writeFileSync(join(build, `${APP_NAME}.exe`), "binary");

  assert.throws(
    () =>
      stageBundle({
        appDir: join(root, "staging", APP_NAME),
        binary: join(build, `${APP_NAME}.exe`),
        resources: join(build, RESOURCES_DIR),
        version: "1.2.3",
      }),
    /bundled resources not found/,
  );

  rmSync(root, { recursive: true, force: true });
});
