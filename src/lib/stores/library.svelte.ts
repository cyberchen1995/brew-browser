/**
 * Library tab view state. Lifted out of the component so other surfaces
 * (Dashboard's "Updates available" card, command palette in future) can
 * preset the filter before navigating.
 *
 * The state survives section switches within a session but is not persisted
 * to localStorage — by design. Reloading the app returns to the default
 * "All" view.
 */

export type LibraryFilter =
  | "all"
  | "formulae"
  | "casks"
  | "outdated"
  /** v0.5.0 — show only packages with at least one known vulnerability.
      The pill is hidden in the UI when `vulnerabilities.enabled` is false
      (no point showing a filter for data we don't have); jumping into
      this filter via the Dashboard "View vulnerable packages →" link
      first ensures the feature is enabled. */
  | "vulnerable";

class LibraryStore {
  filter: LibraryFilter = $state("all");

  setFilter(f: LibraryFilter) {
    this.filter = f;
  }
}

export const library = new LibraryStore();
