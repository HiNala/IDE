//! Re-exports shell layout from `editor-layout` for a single import path (`editor_ui::chrome_layout`).

pub use editor_layout::chrome_shell::{
    build_chrome_tree, compute_main_chrome_layout, format_main_chrome_layout_golden,
    main_chrome_to_layout_result, ChromeWidgetId, MainChromeLayout, MainChromeParams,
};
