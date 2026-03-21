mod git_panel;
mod terminal;
mod theme;
mod workspace_view;

use gpui::{px, size, point, Application, Bounds, TitlebarOptions, WindowBounds, WindowOptions};

fn main() {
    Application::new().run(|app| {
        theme::init(app);

        let bounds = Bounds::centered(None, size(px(1280.0), px(800.0)), app);
        app.open_window(
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
        )
        .unwrap();
    });
}
