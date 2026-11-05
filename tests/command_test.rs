use observability::command::*;
use std::time::Duration;

#[test]
fn parses_all_commands() {
    assert_eq!(parse(":dash"), Ok(Command::Dash));
    assert_eq!(parse("proc"), Ok(Command::Proc));
    assert_eq!(parse("sort mem"), Ok(Command::Sort(SortKey::Mem)));
    assert_eq!(
        parse("filter fire fox"),
        Ok(Command::Filter(Some("fire fox".into())))
    );
    assert_eq!(parse("filter"), Ok(Command::Filter(None)));
    assert_eq!(parse("kill 42"), Ok(Command::Kill(42)));
    assert_eq!(
        parse("refresh 500ms"),
        Ok(Command::Refresh(Duration::from_millis(500)))
    );
    assert_eq!(
        parse("refresh 2s"),
        Ok(Command::Refresh(Duration::from_secs(2)))
    );
    assert_eq!(
        parse("refresh 1.5s"),
        Ok(Command::Refresh(Duration::from_millis(1500)))
    );
    assert_eq!(parse("gpu 2"), Ok(Command::Gpu(1)));
    assert_eq!(parse("net wlan0"), Ok(Command::Net(Some("wlan0".into()))));
    assert_eq!(parse("net auto"), Ok(Command::Net(None)));
    assert_eq!(parse("net"), Ok(Command::Net(None)));
    assert_eq!(parse("q"), Ok(Command::Quit));
    assert_eq!(parse("quit"), Ok(Command::Quit));
}

#[test]
fn reports_errors_in_english() {
    assert!(parse("sort").unwrap_err().contains("usage"));
    assert!(parse("sort foo").unwrap_err().contains("foo"));
    assert!(parse("kill abc").unwrap_err().contains("pid"));
    assert!(parse("refresh 50ms").unwrap_err().contains("100ms"));
    assert!(parse("refresh 2m").is_err());
    assert!(parse("gpu 0").is_err());
    assert!(parse("").is_err());
    assert!(parse("foo").unwrap_err().contains("unknown"));
}

#[test]
fn completes_commands_and_arguments() {
    assert_eq!(complete("").len(), 9);
    assert_eq!(complete("s"), vec!["sort"]);
    assert_eq!(complete(":re"), vec!["refresh"]);
    assert_eq!(complete("sort ").len(), 5);
    assert_eq!(complete("sort c"), vec!["cpu"]);
    assert_eq!(complete("net a"), vec!["auto"]);
    assert!(complete("kill ").is_empty());
}

#[test]
fn applies_unique_completion() {
    assert_eq!(apply_completion("so"), Some("sort ".into()));
    assert_eq!(apply_completion("sort m"), Some("sort mem".into()));
    assert_eq!(apply_completion("r"), Some("refresh ".into()));
    assert_eq!(apply_completion(""), None);
}

#[test]
fn sort_key_labels() {
    assert_eq!(SortKey::Cpu.label(), "cpu");
    assert_eq!(SortKey::Name.label(), "name");
}

#[test]
fn kill_rejects_out_of_range_pids() {
    for bad in ["kill 0", "kill 4294967295", "kill 2147483648"] {
        assert!(parse(bad).unwrap_err().contains("pid"), "{bad}");
    }
    assert_eq!(parse("kill 2147483647"), Ok(Command::Kill(2147483647)));
    assert_eq!(parse("kill 1"), Ok(Command::Kill(1)));
}
