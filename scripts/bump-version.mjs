#!/usr/bin/env node
// Writes one semantic version into every file that duplicates it.
//
// The version lives in four places that can silently drift apart
// (package.json, package-lock.json, tauri.conf.json, Cargo.toml). This is the
// single writer for all of them; the release workflow never edits them by hand.
//
// Usage:
//   node scripts/bump-version.mjs <patch|minor|major> [--base X.Y.Z] [--dry-run]
//   node scripts/bump-version.mjs X.Y.Z [--dry-run]
//
// Prints ONLY the resulting version to stdout, so the workflow can capture it.

import { readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..");

const SEMVER = /^(\d+)\.(\d+)\.(\d+)$/;
const BUMPS = new Set(["major", "minor", "patch"]);

export function parseVersion(raw) {
  const match = SEMVER.exec(String(raw ?? "").trim());
  if (!match) throw new Error(`invalid semantic version: ${JSON.stringify(raw)}`);
  return { major: Number(match[1]), minor: Number(match[2]), patch: Number(match[3]) };
}

export function bumpVersion(current, kind) {
  if (!BUMPS.has(kind)) throw new Error(`unknown bump kind: ${JSON.stringify(kind)}`);
  const { major, minor, patch } = parseVersion(current);
  if (kind === "major") return `${major + 1}.0.0`;
  if (kind === "minor") return `${major}.${minor + 1}.0`;
  return `${major}.${minor}.${patch + 1}`;
}

/** Strips a leading `v` so git tags and package versions can be compared. */
export function stripTagPrefix(tag) {
  const trimmed = String(tag ?? "").trim();
  return trimmed.startsWith("v") ? trimmed.slice(1) : trimmed;
}

export function setJsonVersion(source, version) {
  const data = JSON.parse(source);
  data.version = version;
  // package-lock.json repeats the version inside the root package entry.
  if (data.packages && data.packages[""]) data.packages[""].version = version;
  return `${JSON.stringify(data, null, 2)}\n`;
}

/**
 * Rewrites `version` inside `[package]` only. A naive global replace would also
 * hit dependency versions further down the file.
 */
export function setCargoVersion(source, version) {
  const start = source.indexOf("[package]");
  if (start === -1) throw new Error("Cargo.toml: [package] section not found");

  const relativeEnd = source.slice(start + 1).search(/\n\[/);
  const end = relativeEnd === -1 ? source.length : start + 1 + relativeEnd;

  const section = source.slice(start, end);
  const patched = section.replace(/^version\s*=\s*"[^"]*"/m, `version = "${version}"`);
  if (patched === section) throw new Error("Cargo.toml: version key not found in [package]");

  return source.slice(0, start) + patched + source.slice(end);
}

const TARGETS = [
  { file: "package.json", apply: setJsonVersion },
  { file: "package-lock.json", apply: setJsonVersion },
  { file: "src-tauri/tauri.conf.json", apply: setJsonVersion },
  { file: "src-tauri/Cargo.toml", apply: setCargoVersion },
];

export function readCurrentVersion(root = ROOT) {
  return JSON.parse(readFileSync(join(root, "package.json"), "utf8")).version;
}

export function applyVersion(version, { root = ROOT, dryRun = false } = {}) {
  parseVersion(version); // reject garbage before touching anything
  const written = [];
  for (const target of TARGETS) {
    const path = join(root, target.file);
    const source = readFileSync(path, "utf8");
    const next = target.apply(source, version);
    if (!dryRun) writeFileSync(path, next);
    written.push(target.file);
  }
  return written;
}

function parseArgs(argv) {
  const positional = [];
  const flags = { dryRun: false, base: null };
  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    if (arg === "--dry-run") flags.dryRun = true;
    else if (arg === "--base") flags.base = argv[++i];
    else positional.push(arg);
  }
  return { positional, flags };
}

function main(argv) {
  const { positional, flags } = parseArgs(argv);
  const target = positional[0];
  if (!target) {
    throw new Error("usage: bump-version.mjs <patch|minor|major|X.Y.Z> [--base X.Y.Z] [--dry-run]");
  }

  const version = SEMVER.test(target)
    ? target
    : bumpVersion(flags.base ? stripTagPrefix(flags.base) : readCurrentVersion(), target);

  applyVersion(version, { dryRun: flags.dryRun });
  process.stdout.write(`${version}\n`);
}

// Only run when invoked directly, so the test file can import the pure helpers.
if (process.argv[1] && process.argv[1].endsWith("bump-version.mjs")) {
  try {
    main(process.argv.slice(2));
  } catch (error) {
    process.stderr.write(`${error.message}\n`);
    process.exit(1);
  }
}
