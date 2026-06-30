use gpui::{
    AppContext, Context, Entity, EventEmitter, ParentElement, Render, SharedString, Styled, Window,
    div, px,
};
use gpui_component::{
    ActiveTheme, IndexPath,
    button::{Button, ButtonVariants},
    input::{Input, InputState},
    select::{Select, SelectState},
    separator::Separator,
};

use crate::routes::Navigate;

pub struct HomeScreen {
    input_state: Entity<InputState>,
    select_state: Entity<SelectState<Vec<SharedString>>>,
}

impl HomeScreen {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let select_state = cx.new(|cx| {
            SelectState::new(
                vec!["GET".into(), "POST".into(), "PUT".into(), "DELETE".into()],
                Some(IndexPath::default()),
                window,
                cx,
            )
        });
        Self {
            input_state: cx.new(|cx| InputState::new(window, cx)),
            select_state,
        }
    }
}

impl EventEmitter<Navigate> for HomeScreen {}

impl Render for HomeScreen {
    fn render(
        &mut self,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl gpui::prelude::IntoElement {
        div()
            .flex()
            .flex_col()
            .w_full()
            .h_full()
            .p_4()
            .child(
                div()
                    .flex()
                    .border_1()
                    .rounded(cx.theme().radius)
                    .border_color(cx.theme().input)
                    .text_color(cx.theme().secondary_foreground)
                    .w_full()
                    .child(
                        div()
                            .w(px(140.))
                            .child(Select::new(&self.select_state).appearance(false)),
                    )
                    .child(Separator::vertical())
                    .child(
                        div()
                            .flex_1()
                            .child(Input::new(&self.input_state).appearance(false)),
                    )
                    .child(div().child(Button::new("send").ghost().label("Send"))),
            )
            .child("hello world")
    }
}
