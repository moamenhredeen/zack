use gpui::{EventEmitter, ParentElement, Render, Styled, div, rems};
use gpui_component::label::Label;

use crate::routes::Navigate;

pub struct SettingsScreen;

impl EventEmitter<Navigate> for SettingsScreen {}

impl Render for SettingsScreen {
    fn render(
        &mut self,
        _: &mut gpui::Window,
        _: &mut gpui::prelude::Context<Self>,
    ) -> impl gpui::prelude::IntoElement {
        div().flex().flex_col().p_2().gap_y_2().child(
            Label::new("Settings Screen")
                .text_size(rems(2.5))
                .text_center(),
        )
    }
}
