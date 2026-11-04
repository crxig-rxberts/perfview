# What does this change?

<!-- A short description of the change and why it is needed. -->

## Related issue

<!-- e.g. Closes #12 -->

## Screenshots

<!--
If this changes anything visible, please include a before and after.
`./scripts/capture-screenshots.sh` can produce consistent captures.
-->

| Before | After |
| ------ | ----- |
|        |       |

## Checklist

- [ ] `cargo fmt --all` leaves nothing to change
- [ ] `cargo clippy --all-targets --all-features -- -D warnings` is clean
- [ ] `cargo test` passes
- [ ] Any new reading is **translated into plain English** — no raw kernel
      counters, no unexplained abbreviations, and a hover hint that says what
      it means
- [ ] Any sensor that may be missing degrades to "Not available" rather than
      showing a misleading zero
- [ ] The commit messages follow [Conventional Commits](https://www.conventionalcommits.org)
