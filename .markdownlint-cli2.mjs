export default {
  config: {
    default: true,
    // Long prose lines are fine; the source is meant to be read in an editor
    // that soft-wraps.
    MD013: false,
    // The README deliberately uses a little HTML for the centred header badge
    // block, the screenshots and the collapsible sections. These are the only
    // elements allowed.
    MD033: {
      allowed_elements: ["br", "details", "div", "img", "kbd", "strong", "summary"],
    },
    // The README opens with the centred header block rather than a bare `#`.
    MD041: false,
  },
  globs: ["**/*.md"],
  ignores: ["target/**", "node_modules/**", "CHANGELOG.md"],
};
