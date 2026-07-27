import assert from "node:assert/strict";
import { test } from "node:test";

import { findEntries, parseDpkgLine } from "./check-linux-bundle.mjs";

/// Real `dpkg -c` output shape, captured from a Tauri .deb listing.
const LISTING = [
  "drwxr-xr-x root/root         0 2026-07-27 10:00 ./usr/lib/LocalMind/resources/llama/vulkan/",
  "-rwxr-xr-x root/root   5242880 2026-07-27 10:00 ./usr/lib/LocalMind/resources/llama/vulkan/llama-server",
  "-rw-r--r-- root/root   5242880 2026-07-27 10:00 ./usr/lib/LocalMind/resources/llama/cpu/llama-server",
  "-rwxr-xr-x root/root  15809848 2026-07-27 10:00 ./usr/lib/LocalMind/resources/onnxruntime/lib/libonnxruntime.so",
  "-rw-r--r-- root/root       412 2026-07-27 10:00 ./usr/share/applications/LocalMind.desktop",
].join("\n");

test("a mode with x is read as executable and one without is not", () => {
  const withBit = parseDpkgLine(LISTING.split("\n")[1]);
  assert.equal(withBit.executable, true);
  assert.equal(withBit.mode, "-rwxr-xr-x");
  assert.equal(withBit.size, 5242880);
  assert.match(withBit.path, /llama\/vulkan\/llama-server$/);

  const withoutBit = parseDpkgLine(LISTING.split("\n")[2]);
  assert.equal(withoutBit.executable, false);
  assert.equal(withoutBit.mode, "-rw-r--r--");
});

test("directory lines and blank lines do not become entries", () => {
  assert.equal(parseDpkgLine(LISTING.split("\n")[0]).path.endsWith("/"), true);
  assert.equal(parseDpkgLine(""), null);
  assert.equal(parseDpkgLine("garbage"), null);
});

test("both llama-server copies are found, not just the first", () => {
  const found = findEntries(LISTING, "llama-server");
  assert.equal(found.length, 2, "the Vulkan and CPU backends both ship");
  assert.deepEqual(
    found.map((e) => e.executable),
    [true, false],
  );
});

test("a package without the component reports nothing rather than a false match", () => {
  assert.deepEqual(findEntries(LISTING, "libpdfium.so"), []);
  // `llama-server` must not match `llama-server-extra`.
  assert.deepEqual(findEntries("-rwxr-xr-x root/root 1 d t ./x/llama-server-extra", "llama-server"), []);
});
