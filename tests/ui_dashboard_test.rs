mod common;

use common::*;
use guaitop::app::Panel;
use guaitop::model::Avail;
use guaitop::ui::{compact, dashboard, theme::Theme};

fn theme() -> Theme {
    Theme::new(true, 60.0, 80.0)
}

#[test]
fn dashboard_layout_b_shows_every_panel() {
    let app = sample_app(60);
    let text = render_text(122, 36, |f| dashboard::render(f, f.area(), &app, &theme()));
    for needle in [
        "CPU",
        "MEMORY",
        "GPU",
        "DISK /",
        "HISTORY CPU · GPU",
        "TEMPERATURES",
        "NETWORK · enp0s31f6",
        "DISK I/O",
    ] {
        assert!(text.contains(needle), "falta {needle}\n{text}");
    }
    assert!(text.contains("Core Ultra 5 125H"));
    assert!(text.contains("14 threads · 3.1 GHz"));
    assert!(text.contains("66%"), "disk 312/472");
    assert!(text.contains("68°C"));
    assert!(text.contains("↓ ") && text.contains("↑ "), "{text}");
    insta::assert_snapshot!(text);
}

#[test]
fn gpu_panel_explains_missing_permission() {
    let mut app = sample_app(2);
    if let Avail::Ok(g) = &mut app.snapshot.gpus {
        g[0].util = Avail::Unavailable(
            "permission needed to read the Intel GPU: sudo setcap cap_perfmon=ep /usr/bin/guaitop"
                .into(),
        );
    }
    let text = render_text(122, 36, |f| dashboard::render(f, f.area(), &app, &theme()));
    assert!(text.contains("sudo setcap"), "{text}");
    assert!(text.contains("cap_perfmon=ep"), "{text}");
}

#[test]
fn unavailable_temps_and_no_gpu_show_reasons() {
    let mut app = sample_app(2);
    app.snapshot.temps = Avail::Unavailable("no hwmon sensor found".into());
    app.snapshot.gpus = Avail::Unavailable("no GPU detected".into());
    let text = render_text(122, 36, |f| dashboard::render(f, f.area(), &app, &theme()));
    assert!(text.contains("no hwmon sensor found"));
    assert!(text.contains("no GPU detected"));
}

#[test]
fn zoom_renders_only_the_focused_panel_with_legend() {
    let mut app = sample_app(10);
    app.focus = Panel::MemDonut;
    app.zoomed = true;
    let text = render_text(122, 36, |f| dashboard::render(f, f.area(), &app, &theme()));
    assert!(text.contains("MEMORY"));
    assert!(!text.contains("TEMPERATURES"));
    for label in ["apps", "cache", "buffers", "free"] {
        assert!(text.contains(label), "missing legend {label}");
    }
}

#[test]
fn zoomed_legend_keeps_a_gap_from_the_donut() {
    let mut app = sample_app(10);
    app.focus = Panel::MemDonut;
    app.zoomed = true;
    for aspect in [2.0, 2.2, 2.5] {
        let mut th = theme();
        th.cell_aspect = aspect;
        let text = render_text(122, 36, |f| dashboard::render(f, f.area(), &app, &th));
        let rows: Vec<Vec<char>> = text.lines().map(|l| l.chars().collect()).collect();
        let legend_x = rows
            .iter()
            .find_map(|r| r.iter().position(|c| *c == '■'))
            .expect("legend");
        // Rightmost column of the donut (quadrant blocks), on any row.
        let pie_right = rows
            .iter()
            .filter_map(|r| {
                r[..legend_x]
                    .iter()
                    .rposition(|c| "▘▝▀▖▌▞▛▗▚▐▜▄▙▟█".contains(*c))
            })
            .max()
            .expect("donut");
        assert!(
            legend_x - pie_right >= 3,
            "aspect {aspect}: donut up to column {pie_right}, legend at {legend_x}\n{text}"
        );
    }
}

#[test]
fn panels_keep_inner_padding_and_donut_caption_sits_under_the_circle() {
    let app = sample_app(10);
    let text = render_text(122, 36, |f| dashboard::render(f, f.area(), &app, &theme()));
    let rows: Vec<Vec<char>> = text.lines().map(|l| l.chars().collect()).collect();
    let blocks = "▘▝▀▖▌▞▛▗▚▐▜▄▙▟█";
    // CPU panel: columns 0..30 and the rows of the top strip (down to the bottom edge).
    // No donut block stuck to the edge (column 1 or 29).
    let bottom = (1..rows.len())
        .find(|y| rows[*y][0] == '╰')
        .expect("border");
    let mut pie_rows = Vec::new();
    for (y, r) in rows.iter().enumerate().take(bottom) {
        let line: String = r[..30].iter().collect();
        if line[3..].contains(|c| blocks.contains(c)) {
            pie_rows.push(y);
            assert!(
                !blocks.contains(r[1]) && !blocks.contains(r[29]),
                "row {y}: {line}"
            );
        }
    }
    let caption = rows
        .iter()
        .position(|r| r.iter().collect::<String>().contains("14 threads"))
        .expect("caption");
    let last = *pie_rows.last().expect("donut");
    assert!(
        caption > last && caption - last <= 2,
        "caption at row {caption}, donut ends at {last}\n{text}"
    );
    // Top margin of the donut: it does not start on the first row of the panel.
    assert!(pie_rows[0] >= 2, "{text}");
}

#[test]
fn short_model_strips_marketing() {
    assert_eq!(
        dashboard::short_model("Intel(R) Core(TM) Ultra 5 125H"),
        "Core Ultra 5 125H"
    );
    assert_eq!(dashboard::short_model("Apple M3 Pro"), "Apple M3 Pro");
    assert_eq!(
        dashboard::short_model("AMD Ryzen 9 7950X 16-Core Processor"),
        "AMD Ryzen 9 7950X 16-Core"
    );
}

#[test]
fn compact_view_fits_80x24() {
    let app = sample_app(30);
    let text = render_text(80, 22, |f| compact::render(f, f.area(), &app, &theme()));
    for needle in ["SUMMARY", "CPU", "GPU", "MEM", "SWAP", "DISK", "↓", "↑"] {
        assert!(text.contains(needle), "falta {needle}\n{text}");
    }
    insta::assert_snapshot!(text);
}

#[test]
fn tiny_terminals_do_not_panic() {
    let app = sample_app(5);
    render_text(20, 6, |f| dashboard::render(f, f.area(), &app, &theme()));
    render_text(20, 6, |f| compact::render(f, f.area(), &app, &theme()));
    render_text(3, 3, |f| dashboard::render(f, f.area(), &app, &theme()));
    render_text(10, 4, |f| dashboard::render(f, f.area(), &app, &theme()));
}

#[test]
fn evolution_charts_have_scales_and_time_axis() {
    let app = sample_app(60);
    let text = render_text(122, 40, |f| dashboard::render(f, f.area(), &app, &theme()));
    // History: percentage scale on CPU and GPU.
    assert!(text.matches("100%┤").count() >= 2, "{text}");
    assert!(text.contains(" 0%┤"), "{text}");
    // Time axis: line with ticks and labels up to «ara» (history, network and disk).
    assert!(text.matches('└').count() >= 3, "{text}");
    assert!(text.matches("now").count() >= 3, "{text}");
    assert!(text.contains("-30s") || text.contains("-60s"), "{text}");
    // Network and disk: scale with compact speed and the zero.
    assert!(text.contains("   0┤"), "{text}");
}
