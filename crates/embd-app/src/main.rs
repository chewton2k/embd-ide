mod assets;
mod git_panel;
mod terminal;
mod theme;
mod workspace_view;

use gpui::{px, size, point, Application, Bounds, TitlebarOptions, WindowBounds, WindowOptions};

fn main() {
    let application = Application::new().with_assets(assets::EmbeddedAssets);

    // Re-open window when dock icon is clicked after all windows are closed
    application.on_reopen(|app| {
        open_main_window(app);
    });

    application.run(|app| {
        theme::init(app);
        open_main_window(app);
    });
}

fn open_main_window(app: &mut gpui::App) {
    let bounds = Bounds::centered(None, size(px(1280.0), px(800.0)), app);
    if let Err(e) = app.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: Some(TitlebarOptions {
                title: Some("embd".into()),
                appears_transparent: true,
                traffic_light_position: Some(point(px(9.0), px(9.0))),
            }),
            ..Default::default()
        },
        |window, app| workspace_view::build_workspace(window, app),
    ) {
        eprintln!("Failed to open window: {e}");
    }
}
