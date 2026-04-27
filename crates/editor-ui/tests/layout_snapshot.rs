//! Checked-in goldens for [`editor_layout::compute_main_chrome_layout`] / shell rects.
//! Update snapshots only when shell geometry rules intentionally change.

use editor_ui::{
    compute_main_chrome_layout, format_main_chrome_layout_golden, main_chrome_to_layout_result,
    ChromeWidgetId, MainChromeParams,
};

fn params_960x600() -> MainChromeParams {
    MainChromeParams {
        window_width_px: 960.0,
        window_height_px: 600.0,
        scale: 1.0,
        title_bar_height_logical: 34.0,
        tab_strip_height_logical: 32.0,
        breadcrumbs_height_logical: 24.0,
        show_tab_strip: true,
        show_breadcrumbs: true,
        activity_bar_width_logical: 0.0,
        sidebar_width_logical: 220.0,
        sidebar_visible: true,
        agent_width_logical: 360.0,
        agent_panel_visible: true,
        status_bar_height_px: 24.0,
        terminal_pane_height_px: 160.0,
    }
}

fn params_1920x1080() -> MainChromeParams {
    MainChromeParams {
        window_width_px: 1920.0,
        window_height_px: 1080.0,
        scale: 1.0,
        title_bar_height_logical: 34.0,
        tab_strip_height_logical: 32.0,
        breadcrumbs_height_logical: 24.0,
        show_tab_strip: true,
        show_breadcrumbs: true,
        activity_bar_width_logical: 0.0,
        sidebar_width_logical: 220.0,
        sidebar_visible: true,
        agent_width_logical: 360.0,
        agent_panel_visible: true,
        status_bar_height_px: 24.0,
        terminal_pane_height_px: 200.0,
    }
}

fn params_2560x1440() -> MainChromeParams {
    MainChromeParams {
        window_width_px: 2560.0,
        window_height_px: 1440.0,
        scale: 1.0,
        title_bar_height_logical: 34.0,
        tab_strip_height_logical: 32.0,
        breadcrumbs_height_logical: 24.0,
        show_tab_strip: true,
        show_breadcrumbs: true,
        activity_bar_width_logical: 0.0,
        sidebar_width_logical: 220.0,
        sidebar_visible: true,
        agent_width_logical: 360.0,
        agent_panel_visible: true,
        status_bar_height_px: 24.0,
        terminal_pane_height_px: 220.0,
    }
}

#[test]
fn golden_main_chrome_960x600() {
    let p = params_960x600();
    let l = compute_main_chrome_layout(&p);
    let got = format_main_chrome_layout_golden(&l);
    let expected = include_str!("snapshots/main_chrome_960x600.txt");
    assert_eq!(got, expected, "update snapshots/main_chrome_960x600.txt if layout rules change");
}

#[test]
fn golden_main_chrome_1920x1080() {
    let p = params_1920x1080();
    let l = compute_main_chrome_layout(&p);
    let got = format_main_chrome_layout_golden(&l);
    let expected = include_str!("snapshots/main_chrome_1920x1080.txt");
    assert_eq!(got, expected, "update snapshots/main_chrome_1920x1080.txt if layout rules change");
}

#[test]
fn golden_main_chrome_2560x1440() {
    let p = params_2560x1440();
    let l = compute_main_chrome_layout(&p);
    let got = format_main_chrome_layout_golden(&l);
    let expected = include_str!("snapshots/main_chrome_2560x1440.txt");
    assert_eq!(got, expected, "update snapshots/main_chrome_2560x1440.txt if layout rules change");
}

#[test]
fn layout_result_has_status_and_agent_ids() {
    let r = main_chrome_to_layout_result(&params_1920x1080());
    assert!(r.items.iter().any(|i| i.widget_id == ChromeWidgetId::STATUS_BAR));
    assert!(r.items.iter().any(|i| i.widget_id == ChromeWidgetId::AGENT_PANEL));
    assert!(r.items.iter().any(|i| i.widget_id == ChromeWidgetId::EDITOR_VIEWPORT));
    assert!((r.root_width - 1920.0).abs() < f32::EPSILON);
    assert!((r.root_height - 1080.0).abs() < f32::EPSILON);
}
