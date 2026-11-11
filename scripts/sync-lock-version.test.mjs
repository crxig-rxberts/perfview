import assert from "node:assert/strict";
import { test } from "node:test";
import { syncLockVersion } from "./sync-lock-version.mjs";

const dependency = '[[package]]\nname = "dependency"\nversion = "0.1.0"\nchecksum = "unchanged"\n';
const workspace = '[[package]]\nname = "perfview"\nversion = "0.1.0"\ndependencies = ["dependency"]\n';

test("updates only the workspace version, preserving dependencies and checksums", () => {
  assert.equal(
    syncLockVersion(dependency + workspace, "0.2.0"),
    dependency + workspace.replace('version = "0.1.0"', 'version = "0.2.0"'),
  );
});

test("supports prereleases and CRLF lockfiles", () => {
  assert.equal(
    syncLockVersion(workspace.replaceAll("\n", "\r\n"), "0.2.0-rc.1"),
    workspace.replace('version = "0.1.0"', 'version = "0.2.0-rc.1"').replaceAll("\n", "\r\n"),
  );
});

test("rejects invalid versions and missing or ambiguous workspace entries", () => {
  assert.throws(() => syncLockVersion(workspace, undefined), /Invalid release version/);
  assert.throws(() => syncLockVersion(workspace, '0.2.0"\ninjected'), /Invalid release version/);
  assert.throws(() => syncLockVersion(dependency, "0.2.0"), /exactly one/);
  assert.throws(() => syncLockVersion(workspace + workspace, "0.2.0"), /exactly one/);
});
