// Release configuration for release-it, run from CI on every push to main.
//
// The version lives in Cargo.toml and is mirrored into Cargo.lock, so both are
// bumped together and committed as one release commit.

const packageName = "perfview";

export default {
  git: {
    commitMessage: `chore(release): ${packageName} v\${version}`,
    tagName: "v${version}",
    tagAnnotation: `${packageName} v\${version}`,
    requireCleanWorkingDir: false,
    push: true,
  },
  github: {
    release: true,
    releaseName: "v${version}",
    // Binaries are attached by the release workflow once they are built.
    assets: [],
  },
  npm: {
    publish: false,
  },
  hooks: {
    // Keep Cargo.lock's recorded version in step with Cargo.toml.
    "after:bump": "node scripts/sync-lock-version.mjs ${version}",
  },
  plugins: {
    "@release-it/conventional-changelog": {
      infile: "CHANGELOG.md",
      header: "# Changelog\n\nAll notable changes to perfview are recorded here.\n",
      preset: {
        name: "conventionalcommits",
        types: [
          { type: "feat", section: "Features" },
          { type: "fix", section: "Bug Fixes" },
          { type: "perf", section: "Performance" },
          { type: "refactor", section: "Refactoring" },
          { type: "docs", section: "Documentation" },
          { type: "build", section: "Build System" },
          { type: "ci", section: "Continuous Integration" },
          { type: "chore", section: "Chores" },
          { type: "test", section: "Tests" },
        ],
      },
    },
    "@release-it/bumper": {
      in: { file: "Cargo.toml", path: "package.version" },
      out: [{ file: "Cargo.toml", path: "package.version" }],
    },
  },
};
