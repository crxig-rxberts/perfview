//! Tests for the rolling graph buffers.
//!
//! Exact float comparisons are deliberate here: these values round-trip
//! through the buffer untouched, so any difference at all is a real bug.

#![allow(clippy::float_cmp)]

use perfview::metrics::history::Series;

#[test]
fn keeps_only_the_most_recent_points() {
    let mut series = Series::new(4);
    for i in 0..10 {
        series.push(f64::from(i), f64::from(i) * 10.0);
    }

    let points = series.points();
    assert_eq!(points.len(), 4, "the buffer should not grow beyond its capacity");
    assert_eq!(points.first().unwrap()[0], 6.0, "the oldest points should be dropped");
    assert_eq!(series.latest(), Some(90.0));
}

#[test]
fn summarises_a_series() {
    let mut series = Series::new(16);
    for value in [10.0, 30.0, 20.0, 40.0] {
        series.push(0.0, value);
    }

    assert_eq!(series.max(), 40.0);
    assert_eq!(series.min(), 10.0);
    assert_eq!(series.average(), 25.0);
}

#[test]
fn clamps_negative_readings_to_zero() {
    // Counter wrap-around can briefly produce a negative rate; a graph should
    // show that as zero rather than dipping below the axis.
    let mut series = Series::new(4);
    series.push(0.0, -50.0);

    assert_eq!(series.latest(), Some(0.0));
}

#[test]
fn reports_its_time_span() {
    let mut series = Series::new(8);
    series.push(100.0, 1.0);
    series.push(160.0, 2.0);

    assert_eq!(series.x_range(), Some((100.0, 160.0)));
}

#[test]
fn an_empty_series_is_safe_to_query() {
    let series = Series::new(8);

    assert!(series.is_empty());
    assert_eq!(series.latest(), None);
    assert_eq!(series.x_range(), None);
    assert_eq!(series.average(), 0.0);
    assert_eq!(series.max(), 0.0);
}
