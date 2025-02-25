use super::*;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn test_add_point() {
    let mut ts = FloatTimeSeries::new();
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    ts.add_point(now, 42.0);
    ts.add_point(now + 10, 44.0);
    ts.add_point(now + 5, 43.0);
    ts.add_point(now + 15, 45.0);

    assert_eq!(ts.timestamps.len(), 4);
    assert_eq!(ts.values.len(), 4);
    assert_eq!(ts.values[0], 42.0);
    assert_eq!(ts.values[1], 43.0);
    assert_eq!(ts.values[2], 44.0);
    assert_eq!(ts.values[3], 45.0);
}

#[test]
fn test_get_value_for_timestamp() {
    let mut ts = FloatTimeSeries::new();
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    ts.add_point(now, 42.0);
    ts.add_point(now + 1000, 43.0);

    assert_eq!(ts.get_value_for_timestamp(now), Some(&42.0));
    assert_eq!(ts.get_value_for_timestamp(now + 1000), Some(&43.0));
    assert_eq!(ts.get_value_for_timestamp(now + 2000), None);
}

#[test]
fn test_clear() {
    let mut ts = FloatTimeSeries::new();
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    ts.add_point(now, 42.0);
    ts.add_point(now + 1000, 43.0);

    ts.clear();
    assert_eq!(ts.timestamps.len(), 0);
    assert_eq!(ts.values.len(), 0);
}

#[test]
fn test_float_timeseries_iterator() {
    let mut ts = FloatTimeSeries::new();

    // Add test data
    ts.add_point(1000, 42.0);
    ts.add_point(2000, 43.0);
    ts.add_point(3000, 44.0);

    // Test manual iteration
    let mut iter = ts.iter();
    assert_eq!(iter.next(), Some((1000, &42.0)));
    assert_eq!(iter.next(), Some((2000, &43.0)));
    assert_eq!(iter.next(), Some((3000, &44.0)));
    assert_eq!(iter.next(), None);

    // Test for loop syntax
    let mut count = 0;
    for (ts, val) in &ts {
        count += 1;
        assert!(*val >= 42.0 && *val <= 44.0);
        assert!(ts >= 1000 && ts <= 3000);
    }
    assert_eq!(count, 3);

    // Test collecting into a vector
    let collected: Vec<(u64, &f64)> = ts.iter().collect();
    assert_eq!(collected, vec![(1000, &42.0), (2000, &43.0), (3000, &44.0)]);
}

#[test]
fn test_range_iterator() {
    let mut ts = TimeSeries::new();
    ts.add_point(1000, 10.0);
    ts.add_point(2000, 20.0);
    ts.add_point(3000, 30.0);
    ts.add_point(4000, 40.0);

    // Test full range
    let values: Vec<(u64, &f64)> = ts.range(0, 5000).collect();
    assert_eq!(
        values,
        vec![(1000, &10.0), (2000, &20.0), (3000, &30.0), (4000, &40.0)]
    );

    // Test partial range
    let values: Vec<(u64, &f64)> = ts.range(2000, 3000).collect();
    assert_eq!(values, vec![(2000, &20.0), (3000, &30.0)]);

    // Test empty range
    let values: Vec<(u64, &f64)> = ts.range(2500, 2900).collect();
    assert!(values.is_empty());

    // Test exact boundaries
    let values: Vec<(u64, &f64)> = ts.range(2000, 4000).collect();
    assert_eq!(values, vec![(2000, &20.0), (3000, &30.0), (4000, &40.0)]);

    // Test boundary inclusion/exclusion
    let values: Vec<(u64, &f64)> = ts.range(1999, 4001).collect();
    assert_eq!(values, vec![(2000, &20.0), (3000, &30.0), (4000, &40.0)]);
}

#[test]
fn test_trim() {
    let mut ts = TimeSeries::new();
    ts.add_point(1000, 10.0);
    ts.add_point(2000, 20.0);
    ts.add_point(3000, 30.0);
    ts.add_point(4000, 40.0);

    ts.trim(2000, 3000);

    assert_eq!(ts.timestamps, vec![2000, 3000]);
    assert_eq!(ts.values, vec![20.0, 30.0]);
    assert_eq!(ts.len(), 2);

    ts.trim(2001, 2005);
    assert!(ts.is_empty());
    assert_eq!(ts.len(), 0);
}

#[test]
fn test_serde() {
    let mut ts = FloatTimeSeries::new();
    ts.add_point(1000, 42.0);

    // Test binary format
    let binary = ts.serialize(SerializationFormat::Binary).unwrap();
    let from_binary = FloatTimeSeries::deserialize(&binary, SerializationFormat::Binary).unwrap();
    assert_eq!(from_binary.len(), ts.len());

    // Test JSON format
    // this should return an UnsupportedFormat error
    let json = ts.serialize(SerializationFormat::Json);
    assert!(json.is_err());
}

#[test]
fn test_bucket_iterator() {
    let mut ts = TimeSeries::new();

    // Empty series
    let buckets: Vec<TimeSeries<i32>> = ts.buckets().collect();
    assert_eq!(buckets.len(), 0);

    // Add points in first hour (0-3599)
    ts.add_point(0, 10);
    ts.add_point(1800, 20);
    ts.add_point(3599, 30);

    // Add points in second hour (3600-7199)
    ts.add_point(3600, 40);
    ts.add_point(5400, 50);

    // Add point in third hour (7200-10799)
    ts.add_point(7200, 60);

    // Collect all buckets
    let buckets: Vec<TimeSeries<i32>> = ts.buckets().collect();

    // Should have 3 buckets
    assert_eq!(buckets.len(), 3);

    // First bucket should have 3 points
    assert_eq!(buckets[0].len(), 3);
    assert_eq!(buckets[0].timestamps, vec![0, 1800, 3599]);
    assert_eq!(buckets[0].values, vec![10, 20, 30]);

    // Second bucket should have 2 points
    assert_eq!(buckets[1].len(), 2);
    assert_eq!(buckets[1].timestamps, vec![3600, 5400]);
    assert_eq!(buckets[1].values, vec![40, 50]);

    // Third bucket should have 1 point
    assert_eq!(buckets[2].len(), 1);
    assert_eq!(buckets[2].timestamps, vec![7200]);
    assert_eq!(buckets[2].values, vec![60]);
}

#[test]
fn test_timestamp_key_conversion() {
    // March 15, 2024 14:00 UTC
    let timestamp1: u64 = 1710511200;
    // March 15, 2024 14:30 UTC
    let timestamp2: u64 = 1710511200 + 1800;

    // Convert to key
    let key = TimeSeries::<f64>::ts_to_key(timestamp1);
    assert_eq!(key, "0976091609");

    let key = TimeSeries::<f64>::ts_to_key(timestamp2);
    assert_eq!(key, "0976091609");

    // Convert back to timestamp
    let ts = TimeSeries::<f64>::key_to_ts(&key).unwrap();
    assert_eq!(ts, timestamp1);
}

#[test]
fn test_key_to_ts_invalid_input() {
    assert!(TimeSeries::<f64>::key_to_ts("invalid").is_err());
    assert!(TimeSeries::<f64>::key_to_ts("09760916").is_err());
    assert!(TimeSeries::<f64>::key_to_ts("097x091609").is_err());
}

#[test]
fn test_timeseries_merge() {
    // Test 1: Non-overlapping merge
    let mut ts1 = TimeSeries::<f64>::new();
    ts1.add_point(100, 1.0);
    ts1.add_point(200, 2.0);

    let mut ts2 = TimeSeries::<f64>::new();
    ts2.add_point(300, 3.0);
    ts2.add_point(400, 4.0);

    ts1.merge(&ts2);
    assert_eq!(ts1.len(), 4);
    assert_eq!(*ts1.get_value_for_timestamp(100).unwrap(), 1.0);
    assert_eq!(*ts1.get_value_for_timestamp(400).unwrap(), 4.0);

    // Test 2: Overlapping timestamps
    let mut ts3 = TimeSeries::<f64>::new();
    ts3.add_point(200, 5.0); // Should overwrite
    ts3.add_point(500, 6.0);

    ts1.merge(&ts3);
    assert_eq!(ts1.len(), 5);
    assert_eq!(*ts1.get_value_for_timestamp(200).unwrap(), 5.0); // Verify overwrite

    // Test 3: Empty timeseries
    let empty_ts = TimeSeries::<f64>::new();
    ts1.merge(&empty_ts);
    assert_eq!(ts1.len(), 5); // Should remain unchanged

    // Test 4: Verify ordering
    let timestamps: Vec<u64> = ts1.timestamps.clone();
    assert!(timestamps.windows(2).all(|w| w[0] < w[1]));
}

#[test]
fn test_location_timeseries() {
    let mut series = LocationTimeSeries::new();

    // Add some location points
    series.add_point(1000, LatLong::new(45.5, -122.6)); // Portland coordinates
    series.add_point(2000, LatLong::new(40.7, -74.0)); // NYC coordinates
    series.add_point(3000, LatLong::new(51.5, -0.12)); // London coordinates

    // Test length
    assert_eq!(series.len(), 3);

    // Test get value for timestamp
    let portland = series.get_value_for_timestamp(1000).unwrap();
    assert_eq!(*portland, LatLong::new(45.5, -122.6));

    // Test latest point
    let (ts, london) = series.latest().unwrap();
    assert_eq!(ts, 3000);
    assert_eq!(*london, LatLong::new(51.5, -0.12));

    // Test range iterator
    let range: Vec<_> = series.range(1500, 2500).collect();
    assert_eq!(range.len(), 1);
    assert_eq!(range[0], (2000, &LatLong::new(40.7, -74.0)));

    // Test clear
    series.clear();
    assert!(series.is_empty());
}

#[test]
fn test_metric_value_conversions() {
    // Test float conversions
    let float_val = MetricValue::Float(42.5);
    assert_eq!(float_val.clone().into_float(), Some(42.5));
    assert_eq!(float_val.clone().into_int(), Some(42));
    assert_eq!(float_val.into_location(), None);

    // Test int conversions
    let int_val = MetricValue::Int(42);
    assert_eq!(int_val.clone().into_float(), Some(42.0));
    assert_eq!(int_val.clone().into_int(), Some(42));
    assert_eq!(int_val.into_location(), None);

    // Test location conversions
    let loc_val = MetricValue::Location(LatLong::new(51.5074, -0.1278));
    assert_eq!(loc_val.clone().into_float(), None);
    assert_eq!(loc_val.clone().into_int(), None);
    assert!(loc_val.into_location().is_some());
}

#[test]
fn test_timeseries_conversions() {
    // Create a metric timeseries with float values
    let mut metric_ts = MetricTimeSeries::new();
    metric_ts.add_point(1000, MetricValue::Float(42.5));
    metric_ts.add_point(2000, MetricValue::Float(43.5));

    // Test conversion to float series
    let float_ts = metric_ts.to_float_series().unwrap();
    assert_eq!(float_ts.len(), 2);
    assert_eq!(float_ts.get_value_for_timestamp(1000), Some(&42.5));

    // Test conversion back to metric series
    let converted_metric_ts = MetricTimeSeries::from(&float_ts);
    assert_eq!(converted_metric_ts.len(), 2);
    match converted_metric_ts.get_value_for_timestamp(1000) {
        Some(MetricValue::Float(f)) => assert_eq!(*f, 42.5),
        _ => panic!("Wrong value type"),
    }
}
