use guaitop::history::History;

#[test]
fn keeps_last_values_in_order() {
    let mut h = History::new(3);
    for v in [1.0, 2.0, 3.0, 4.0] {
        h.push(v);
    }
    assert_eq!(h.values(), vec![2.0, 3.0, 4.0]);
    assert_eq!(h.last(), Some(4.0));
    assert_eq!(h.len(), 3);
    assert_eq!(h.capacity(), 3);
    assert_eq!(h.max(), 4.0);
}

#[test]
fn shrinking_capacity_drops_oldest() {
    let mut h = History::new(5);
    for v in 1..=5 {
        h.push(v as f64);
    }
    h.set_capacity(2);
    assert_eq!(h.values(), vec![4.0, 5.0]);
}

#[test]
fn zero_capacity_is_clamped_to_one() {
    let mut h = History::new(0);
    h.push(1.0);
    h.push(2.0);
    assert_eq!(h.values(), vec![2.0]);
    assert!(History::new(4).is_empty());
    assert_eq!(History::new(4).max(), 0.0);
}
