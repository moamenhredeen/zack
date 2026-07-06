mod fonts;
mod http_client;
mod model;
mod opencollection;
mod workspace;
mod settings;

use gpui::*;
use gpui_component::{Root, Theme, TitleBar, v_flex, IconName, h_flex, Selectable, ActiveTheme};
use gpui_component::button::{Button, ButtonVariants};
use crate::fonts::JETBRAINS_MONO;
use crate::workspace::WorkspaceScreen;
use crate::settings::Settings;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Page {
    Collection,
    Environment,
    Settings,
}

pub struct AppShell {
    active_page: Page,
    workspace: Entity<WorkspaceScreen>,
    settings: Entity<Settings>,
}

impl AppShell {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            active_page: Page::Collection,
            workspace: cx.new(|cx| WorkspaceScreen::new(window, cx)),
            settings: cx.new(|cx| Settings::new(window, cx)),
        }
    }

    fn render_activity_bar(&self, cx: &mut Context<Self>) -> impl gpui::IntoElement {
        v_flex()
            .h_full()
            .p_2()
            .gap_2()
            .border_r_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().sidebar)
            .items_center()
            .child(
                Button::new("ab-collection")
                    .ghost()
                    .flex_shrink_0()
                    .icon(IconName::Folder)
                    .selected(self.active_page == Page::Collection)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.active_page = Page::Collection;
                        cx.notify();
                    })),
            )
            .child(
                Button::new("ab-environment")
                    .ghost()
                    .flex_shrink_0()
                    .icon(IconName::File)
                    .selected(self.active_page == Page::Environment)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.active_page = Page::Environment;
                        cx.notify();
                    })),
            )
            .child(
                Button::new("ab-settings")
                    .ghost()
                    .flex_shrink_0()
                    .icon(IconName::Settings)
                    .selected(self.active_page == Page::Settings)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.active_page = Page::Settings;
                        cx.notify();
                    })),
            )
    }

}

impl Render for AppShell {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .size_full()
            .font_family(JETBRAINS_MONO)
            .child(TitleBar::new().child("Zack"))
            .child(
                h_flex()
                    .size_full()
                    .child(self.render_activity_bar(cx))
                    .child(match self.active_page {
                        Page::Collection => self.workspace.clone().into_any_element(),
                        Page::Environment => self.workspace.clone().into_any_element(),
                        Page::Settings => self.settings.clone().into_any_element(),
                    }),
            )
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
            // TODO: is this the correct way to update window bounds
            let bounds = cx
                .update(|cx| WindowBounds::centered(size(px(1200.), px(800.)), cx));

            cx.open_window(
                WindowOptions {
                    titlebar: Some(TitleBar::title_bar_options()),
                    window_decorations: Some(WindowDecorations::Client),
                    window_bounds: Some(bounds),
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
