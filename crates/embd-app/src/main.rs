mod git_panel;
mod search_modal;
mod terminal;
mod theme;
mod workspace_view;

use gpui::{px, size, Application, Bounds, WindowBounds, WindowOptions};

fn main() {
    Application::new().run(|app| {
        theme::init(app);

        let bounds = Bounds::centered(None, size(px(1280.0), px(800.0)), app);
        app.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: None,
                ..Default::default()
            },
            |window, app| workspace_view::build_workspace(window, app),
        )
        .unwrap();
    });
}
