use guaitop::collect::procfs::{Deltas, parse_diskstats, parse_net_dev};

const NET_DEV: &str = "Inter-|   Receive                                                |  Transmit
 face |bytes    packets errs drop fifo frame compressed multicast|bytes    packets errs drop fifo colls carrier compressed
    lo:  123456     100    0    0    0     0          0         0   123456     100    0    0    0     0       0          0
enp0s31f6: 98765432  70000    0    0    0     0          0         0  1234567   9000    0    0    0     0       0          0
docker0:       0       0    0    0    0     0          0         0        0       0    0    0    0     0       0          0
";

#[test]
fn net_dev_gives_rx_tx_bytes_per_interface() {
    let v = parse_net_dev(NET_DEV);
    assert_eq!(v.len(), 3);
    assert_eq!(v[1], ("enp0s31f6".to_string(), 98_765_432, 1_234_567));
    assert_eq!(v[0].0, "lo");
}

const DISKSTATS: &str = " 259       0 nvme0n1 1000 0 4000 0 2000 0 8000 0 0 0 0 0 0 0 0 0 0
 259       2 nvme0n1p2 900 0 3000 0 1900 0 7000 0 0 0 0 0 0 0 0 0 0
 253       0 dm-0 800 0 2048 0 1800 0 1024 0 0 0 0 0 0 0 0 0 0
";

#[test]
fn diskstats_gives_bytes_read_and_written() {
    let d = parse_diskstats(DISKSTATS);
    // 512-byte sectors: fields 6 (read) and 10 (written).
    assert_eq!(d.get("dm-0"), Some(&(2048 * 512, 1024 * 512)));
    assert_eq!(d.get("nvme0n1"), Some(&(4000 * 512, 8000 * 512)));
    assert!(parse_diskstats("brossa\n1 2\n").is_empty());
}

#[test]
fn deltas_between_samples() {
    let mut d = Deltas::default();
    assert_eq!(
        d.update("eth0", (100, 50)),
        (0, 0),
        "first sample: no reference"
    );
    assert_eq!(d.update("eth0", (160, 80)), (60, 30));
    // Counter reset (interface recreated): no negative jump is reported.
    assert_eq!(d.update("eth0", (10, 5)), (0, 0));
    assert_eq!(d.update("eth0", (20, 9)), (10, 4));
}
