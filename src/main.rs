mod fonts;
mod http_client;
mod model;
mod opencollection;
mod workspace;

use gpui::*;
use gpui_component::{Root, Theme, TitleBar, v_flex};

use crate::fonts::JETBRAINS_MONO;
use crate::workspace::WorkspaceScreen;

pub struct AppShell {
    workspace: Entity<WorkspaceScreen>,
}

impl AppShell {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            workspace: cx.new(|cx| WorkspaceScreen::new(window, cx)),
        }
    }
}

impl Render for AppShell {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .size_full()
            .font_family(JETBRAINS_MONO)
            .child(TitleBar::new().child("Zack"))
            .child(self.workspace.clone())
            .children(Root::render_dialog_layer(window, cx))
    }
}

fn main() {
    let app = gpui_platform::application().with_assets(gpui_component_assets::Assets);

    app.run(move |cx| {
        gpui_component::init(cx);
        fonts::register(cx).expect("failed to register JetBrains Mono");
        {
            let theme = Theme::global_mut(cx);
            theme.font_family = JETBRAINS_MONO.into();
            theme.mono_font_family = JETBRAINS_MONO.into();
        }

        cx.spawn(async move |cx| {
            cx.open_window(
                WindowOptions {
                    titlebar: Some(TitleBar::title_bar_options()),
                    window_decorations: Some(WindowDecorations::Client),
                    ..Default::default()
                },
                |window, cx| {
                    let view = cx.new(|cx| AppShell::new(window, cx));
                    cx.new(|cx| Root::new(view, window, cx))
                },
            )
            .expect("Failed to open window");
        })
        .detach();
    });
}
