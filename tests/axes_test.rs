use observability::ui::axes::*;
use observability::ui::theme::Theme;
use ratatui::{buffer::Buffer, layout::Rect};

fn row(buf: &Buffer, y: u16) -> String {
    (buf.area.x..buf.area.right())
        .map(|x| buf[(x, y)].symbol().to_string())
        .collect()
}

#[test]
fn y_axis_reserves_label_column_and_draws_ticks() {
    let theme = Theme::new(true, 60.0, 80.0);
    let area = Rect::new(0, 0, 12, 5);
    let mut buf = Buffer::empty(area);
    let labels = [(0, "100%".to_string()), (2, "50%".into()), (4, "0%".into())];
    let plot = y_axis(&mut buf, area, &labels, &theme);
    assert_eq!(plot, Rect::new(5, 0, 7, 5));
    assert_eq!(&row(&buf, 0)[..], "100%┤       ");
    assert_eq!(&row(&buf, 1)[..], "    │       ");
    assert_eq!(&row(&buf, 2)[..], " 50%┤       ");
    assert_eq!(&row(&buf, 4)[..], "  0%┤       ");
}

#[test]
fn x_axis_marks_relative_time_up_to_now() {
    let theme = Theme::new(true, 60.0, 80.0);
    let area = Rect::new(0, 0, 50, 2);
    let mut buf = Buffer::empty(area);
    // 41-column chart starting at column 5; one sample every 1.5 s.
    x_axis(&mut buf, area, Rect::new(5, 0, 41, 1), 1.5, &theme);
    let line = row(&buf, 0);
    let chars: Vec<char> = line.chars().collect();
    assert_eq!(chars[4], '└');
    for c in [5, 15, 25, 35, 45] {
        assert_eq!(chars[c], '┴', "mark at column {c}: {line}");
    }
    assert_eq!(chars[6], '─');
    let labels = row(&buf, 1);
    let pos = |s: &str| {
        labels
            .find(s)
            .unwrap_or_else(|| panic!("falta {s}: {labels}"))
    };
    assert!(pos("-60s") < pos("-45s") && pos("-45s") < pos("-30s"));
    assert!(pos("-30s") < pos("-15s") && pos("-15s") < pos("now"));
    assert!(labels.trim_end().ends_with("now"));
}

#[test]
fn x_axis_on_narrow_chart_keeps_only_the_ends() {
    let theme = Theme::new(true, 60.0, 80.0);
    let area = Rect::new(0, 0, 14, 2);
    let mut buf = Buffer::empty(area);
    x_axis(&mut buf, area, Rect::new(1, 0, 12, 1), 1.0, &theme);
    let labels = row(&buf, 1);
    assert!(
        labels.contains("-11s") && labels.contains("now"),
        "{labels}"
    );
    assert_eq!(labels.matches('s').count(), 1, "{labels}");
}

#[test]
fn x_axis_ticks_land_on_round_times_at_any_width() {
    // 60 s window with widths that do not divide evenly: the labels must be
    // always the same round figures, not «-16s» or «-31s» from column rounding.
    let theme = Theme::new(true, 60.0, 80.0);
    for w in [23u16, 41, 58, 63, 77] {
        let area = Rect::new(0, 0, w + 6, 2);
        let mut buf = Buffer::empty(area);
        x_axis(
            &mut buf,
            area,
            Rect::new(4, 0, w, 1),
            60.0 / f64::from(w - 1),
            &theme,
        );
        let labels = row(&buf, 1);
        let expected: &[&str] = if w >= 40 {
            &["-60s", "-45s", "-30s", "-15s", "now"]
        } else {
            &["-60s", "-30s", "now"]
        };
        for l in expected {
            assert!(labels.contains(l), "width {w}: missing {l} in «{labels}»");
        }
    }
}

#[test]
fn time_labels() {
    assert_eq!(time_label(0.0), "now");
    assert_eq!(time_label(45.0), "-45s");
    assert_eq!(time_label(90.0), "-90s");
    assert_eq!(time_label(300.0), "-5m");
    assert_eq!(time_label(7200.0), "-2h");
}

#[test]
fn short_rates() {
    assert_eq!(short_rate(0.0), "0");
    assert_eq!(short_rate(512.0), "512B");
    assert_eq!(short_rate(2_500.0), "2.5K");
    assert_eq!(short_rate(13_000_000.0), "13M");
    assert_eq!(short_rate(1_200_000.0), "1.2M");
    assert_eq!(short_rate(3.4e9), "3.4G");
}

#[test]
fn axes_survive_tiny_areas() {
    let theme = Theme::new(true, 60.0, 80.0);
    for (w, h) in [(0, 0), (1, 1), (3, 2), (6, 1)] {
        let area = Rect::new(0, 0, w, h);
        let mut buf = Buffer::empty(area);
        let plot = y_axis(&mut buf, area, &[(0, "100%".into())], &theme);
        assert!(plot.width <= w);
        x_axis(&mut buf, area, plot, 1.0, &theme);
    }
}

#[test]
fn fit_series_spans_the_whole_window() {
    // History of 4 samples in 8 columns: each sample takes 2 columns.
    let v = fit_series(&[0.1, 0.2, 0.3, 0.4], 4, 8);
    assert_eq!(v, vec![0.1, 0.1, 0.2, 0.2, 0.3, 0.3, 0.4, 0.4]);
    // History still half empty: the old part has no data (NaN), right-aligned.
    let v = fit_series(&[0.5, 0.6], 4, 4);
    assert!(v[0].is_nan() && v[1].is_nan());
    assert_eq!(&v[2..], &[0.5, 0.6]);
    // More samples than columns: the maximum of each chunk, so peaks are not lost.
    let v = fit_series(&[0.1, 0.9, 0.2, 0.3, 0.0, 0.4], 6, 3);
    assert_eq!(v, vec![0.9, 0.3, 0.4]);
    assert!(fit_series(&[1.0], 0, 5).iter().all(|x| x.is_nan()));
    assert!(fit_series(&[1.0], 3, 0).is_empty());
}
