use gpui::{
    AppContext, Context, Entity, IntoElement, ParentElement, Render, SharedString, Styled, Window,
    div, px,
};
use gpui_component::{
    ActiveTheme, IndexPath, Theme, ThemeMode, h_flex, v_flex,
    select::{Select, SelectEvent, SelectState},
};

pub struct Settings {
    theme_select: Entity<SelectState<Vec<SharedString>>>,
}

impl Settings {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let modes: Vec<SharedString> = vec!["Light".into(), "Dark".into()];
        let selected_index = if cx.theme().mode.is_dark() { 1 } else { 0 };

        let theme_select = cx.new(|cx| {
            SelectState::new(modes, Some(IndexPath::new(selected_index)), window, cx)
        });

        cx.subscribe_in(&theme_select, window, Self::on_theme_select)
            .detach();

        Self { theme_select }
    }

    fn on_theme_select(
        _this: &mut Self,
        _state: &Entity<SelectState<Vec<SharedString>>>,
        event: &SelectEvent<Vec<SharedString>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let SelectEvent::Confirm(Some(value)) = event {
            let mode = if value.as_ref() == "Dark" {
                ThemeMode::Dark
            } else {
                ThemeMode::Light
            };
            Theme::change(mode, Some(window), cx);
        }
    }
}

impl Render for Settings {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .justify_center()
            .items_center()
            .size_full()
            .gap_3()
            .child("Settings")
            .child(
                h_flex()
                    .gap_2()
                    .items_center()
                    .child("Theme")
                    .child(
                        div()
                            .w(px(160.))
                            .child(Select::new(&self.theme_select).placeholder("Theme")),
                    ),
            )
    }
}
