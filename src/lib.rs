//! perfview - a live system performance viewer for Linux.
//!
//! The crate is split into two halves:
//!
//! * [`metrics`] gathers raw numbers from `/proc`, `/sys` and NVML on a
//!   background thread. It knows nothing about the UI.
//! * [`ui`] turns those raw numbers into something a person can read at a
//!   glance, and knows nothing about where they came from.
//!
//! [`app`] glues the two together.

pub mod app;
pub mod metrics;
pub mod ui;

pub use app::{PerfView, Settings, Tab};
