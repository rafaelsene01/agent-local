#!/usr/bin/env node
// Builds the Windows portable bundle: the app in a folder you can copy anywhere,
// with a marker file that tells the running app it is in portable mode.
//
// Tauri has no portable bundle target, and the official updater does not support
// one either — this archive is what the in-app portable updater downloads and
// swaps in. See .specs/features/release-distribution/design.md.
//
// Usage:
//   node scripts/make-portable.mjs --version 1.2.3 [--binary <path>] [--out <dir>]

import { copyFileSync, cpSync, existsSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
import { execFileSync } from "node:child_process";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..");

export const APP_NAME = "LocalMind";
/** Read by `update::flavor()` in the Rust backend. Keep both sides in sync. */
export const PORTABLE_MARKER = ".portable";

/** Must match `bundle.resources` in tauri.conf.json and the folder
 *  `runtime::bundled::resource_root` resolves to. */
export const RESOURCES_DIR = "resources";

export function portableArchiveName(version, arch = "x64") {
  if (!/^\d+\.\d+\.\d+$/.test(String(version ?? ""))) {
    throw new Error(`invalid version: ${JSON.stringify(version)}`);
  }
  return `${APP_NAME}_${version}_${arch}-portable.zip`;
}

export function portableReadme(version) {
  return [
    `${APP_NAME} ${version} — versão portátil`,
    "",
    "1. Extraia esta pasta para qualquer lugar onde você tenha permissão de escrita",
    `   (Documentos, Desktop, um pendrive). Não precisa de administrador.`,
    `2. Execute ${APP_NAME}.exe.`,
    "3. Seus dados ficam em ./data, ao lado do executável — nada é gravado em",
    "   %APPDATA% e nada é escrito no registro do Windows.",
    "",
    `Não apague o arquivo ${PORTABLE_MARKER}: é ele que mantém o app em modo`,
    "portátil e permite que as atualizações sejam aplicadas sem instalação.",
    "",
  ].join("\n");
}

function zipDirectory(sourceDir, destination) {
  // Compress-Archive ships with Windows and needs no extra tooling on the
  // runner. The portable bundle is Windows-only, so there is no other branch.
  execFileSync(
    "powershell",
    [
      "-NoProfile",
      "-NonInteractive",
      "-Command",
      `Compress-Archive -Path '${sourceDir}' -DestinationPath '${destination}' -Force`,
    ],
    { stdio: "inherit" },
  );
}

function parseArgs(argv) {
  const flags = {};
  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    if (arg.startsWith("--")) flags[arg.slice(2)] = argv[++i];
  }
  return flags;
}

/** Lays out the folder that becomes the archive. Split from `main` so the
 *  contents can be asserted without running the zipper. */
export function stageBundle({ appDir, binary, resources, version }) {
  if (!existsSync(binary)) {
    throw new Error(
      `binary not found: ${binary}\n` +
        "Run the Tauri build first, and check that tauri.conf.json sets mainBinaryName.",
    );
  }
  // The runtime components live next to the executable on Windows — that is
  // where Tauri's resource resolver looks. A bundle without them opens and then
  // fails at the first thing the user tries, so its absence is an error here
  // rather than a silently smaller zip (SELF-16).
  if (!existsSync(resources)) {
    throw new Error(
      `bundled resources not found: ${resources}\n` +
        "Run `npm run vendor` and the Tauri build before packaging the portable bundle.",
    );
  }

  mkdirSync(appDir, { recursive: true });
  copyFileSync(binary, join(appDir, `${APP_NAME}.exe`));
  writeFileSync(join(appDir, PORTABLE_MARKER), "");
  writeFileSync(join(appDir, "README.txt"), portableReadme(version));
  cpSync(resources, join(appDir, RESOURCES_DIR), { recursive: true });
  return appDir;
}

function main(argv) {
  const flags = parseArgs(argv);
  const version = flags.version;
  if (!version) throw new Error("--version is required");

  const binary = resolve(flags.binary ?? join(ROOT, "src-tauri", "target", "release", `${APP_NAME}.exe`));
  const outDir = resolve(flags.out ?? join(ROOT, "src-tauri", "target", "release", "portable"));
  const stagingRoot = join(outDir, "staging");
  const appDir = join(stagingRoot, APP_NAME);

  rmSync(stagingRoot, { recursive: true, force: true });
  stageBundle({
    appDir,
    binary,
    resources: resolve(flags.resources ?? join(dirname(binary), RESOURCES_DIR)),
    version,
  });

  const archive = join(outDir, portableArchiveName(version));
  rmSync(archive, { force: true });
  zipDirectory(appDir, archive);
  rmSync(stagingRoot, { recursive: true, force: true });

  // stdout is consumed by the workflow — keep it to the path alone.
  process.stdout.write(`${archive}\n`);
}

if (process.argv[1] && process.argv[1].endsWith("make-portable.mjs")) {
  try {
    main(process.argv.slice(2));
  } catch (error) {
    process.stderr.write(`${error.message}\n`);
    process.exit(1);
  }
}
