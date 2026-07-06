use gpui::{div, Context, IntoElement, ParentElement, Render, Styled, Window};
use gpui_component::v_flex;

pub struct Settings {
}

impl Settings {
    pub(crate) fn new(_window: &mut Window, _cx: &mut Context<Self>) -> Self {
        Self{}
    }
}


impl Render for Settings {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .justify_center()
            .items_center()
            .size_full()
            .child("Hello world")
    }
}