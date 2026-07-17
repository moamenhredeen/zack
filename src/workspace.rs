use std::{
    path::{Path, PathBuf},
    time::Duration,
};

use gpui::{
    AppContext, Context, Entity, Hsla, InteractiveElement, IntoElement, ParentElement, Render,
    SharedString, StatefulInteractiveElement, Styled, Window, div, prelude::FluentBuilder, px,
    relative,
};
use gpui_component::{
    ActiveTheme, IconName, IndexPath, Selectable, Sizable, StyledExt, blue_300,
    button::{Button, ButtonGroup, ButtonVariants, DropdownButton},
    clipboard::Clipboard,
    green_300, h_flex,
    input::{Input, InputEvent, InputState},
    label::Label,
    menu::PopupMenuItem,
    orange_300, red_300,
    scroll::ScrollableElement,
    select::{Select, SelectEvent, SelectState},
    separator::Separator,
    tab::{Tab, TabBar},
    v_flex,
};

use crate::{
    fonts::JETBRAINS_MONO,
    http_client,
    model::{BodyMode, HttpMethod, KeyValueRow, RequestDraft, ResponseRecord},
    opencollection::{self, LoadedCollection},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RequestTab {
    Headers,
    Params,
    Body,
}

impl RequestTab {
    fn index(self) -> usize {
        match self {
            Self::Headers => 0,
            Self::Params => 1,
            Self::Body => 2,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ResponseTab {
    Body,
    Headers,
    Meta,
}

impl ResponseTab {
    fn index(self) -> usize {
        match self {
            Self::Body => 0,
            Self::Headers => 1,
            Self::Meta => 2,
        }
    }
}

pub struct WorkspaceScreen {
    collection: Option<LoadedCollection>,
    selected_path: Option<PathBuf>,
    draft: RequestDraft,
    response: Option<ResponseRecord>,
    status_message: Option<String>,
    is_sending: bool,
    is_dirty: bool,
    request_tab: RequestTab,
    response_tab: ResponseTab,
    response_body_pretty: bool,
    http_client: reqwest::blocking::Client,
    method_select: Entity<SelectState<Vec<SharedString>>>,
    url_input: Entity<InputState>,
    headers_input: Entity<InputState>,
    params_input: Entity<InputState>,
    body_input: Entity<InputState>,
}

impl WorkspaceScreen {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let method_select = cx.new(|cx| {
            SelectState::new(
                HttpMethod::ALL
                    .into_iter()
                    .map(|method| SharedString::from(method.as_str()))
                    .collect::<Vec<_>>(),
                Some(IndexPath::default()),
                window,
                cx,
            )
        });
        let url_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("https://api.example.com/users")
                .default_value("https://httpbin.org/get")
        });
        let headers_input = cx.new(|cx| {
            InputState::new(window, cx)
                .multi_line(true)
                .placeholder("Accept: application/json")
        });
        let params_input = cx.new(|cx| {
            InputState::new(window, cx)
                .multi_line(true)
                .placeholder("page: 1")
        });
        let body_input = cx.new(|cx| {
            InputState::new(window, cx)
                .code_editor("json")
                .line_number(true)
                .placeholder("{\n  \"name\": \"Ada\"\n}")
        });

        cx.subscribe_in(&url_input, window, Self::on_url_input)
            .detach();
        cx.subscribe_in(&method_select, window, Self::on_method_select)
            .detach();
        cx.subscribe_in(&headers_input, window, Self::on_headers_input)
            .detach();
        cx.subscribe_in(&params_input, window, Self::on_params_input)
            .detach();
        cx.subscribe_in(&body_input, window, Self::on_body_input)
            .detach();

        let mut this = Self {
            collection: None,
            selected_path: None,
            draft: RequestDraft::default(),
            response: None,
            status_message: None,
            is_sending: false,
            is_dirty: false,
            request_tab: RequestTab::Headers,
            response_tab: ResponseTab::Body,
            response_body_pretty: true,
            http_client: reqwest::blocking::Client::new(),
            method_select,
            url_input,
            headers_input,
            params_input,
            body_input,
        };

        let default_path = default_collection_path();
        this.load_collection(default_path, window, cx);
        this
    }

    fn load_collection(
        &mut self,
        root: impl AsRef<Path>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match opencollection::ensure_collection_root(root.as_ref())
            .and_then(|_| opencollection::load_collection(root.as_ref()))
        {
            Ok(collection) => {
                self.selected_path = collection
                    .requests
                    .first()
                    .map(|request| request.path.clone());
                self.collection = Some(collection);
                self.load_selected_draft(window, cx);
                self.status_message = Some("Loaded OpenCollection sample".to_string());
            }
            Err(error) => {
                self.collection = None;
                self.selected_path = None;
                self.status_message = Some(error.to_string());
                self.sync_inputs_from_draft(window, cx);
            }
        }
        cx.notify();
    }

    fn select_request(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        if self.is_dirty {
            let _ = self.save_selected();
        }
        self.selected_path = Some(path);
        self.response = None;
        self.load_selected_draft(window, cx);
        cx.notify();
    }

    fn load_selected_draft(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let (Some(collection), Some(path)) = (&self.collection, &self.selected_path)
            && let Some(request) = collection.selected_request(path)
        {
            self.draft = request.draft.clone();
            self.status_message = request.parse_error.clone();
            self.is_dirty = false;
            self.sync_inputs_from_draft(window, cx);
            return;
        }

        self.draft = RequestDraft::default();
        self.is_dirty = false;
        self.sync_inputs_from_draft(window, cx);
    }

    fn save_selected(&mut self) -> anyhow::Result<()> {
        let collection = self
            .collection
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("No collection is loaded"))?;
        let path = self
            .selected_path
            .clone()
            .ok_or_else(|| anyhow::anyhow!("No request is selected"))?;
        let request = collection
            .selected_request_mut(&path)
            .ok_or_else(|| anyhow::anyhow!("Selected request is not in the collection"))?;
        request.draft = self.draft.clone();
        opencollection::save_request(request)?;
        self.is_dirty = false;
        self.status_message = Some("Saved request".to_string());
        Ok(())
    }

    fn create_request(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(collection) = self.collection.as_mut() else {
            self.status_message = Some("Load a collection before creating requests".to_string());
            cx.notify();
            return;
        };

        match opencollection::create_request(&collection.root, "New request") {
            Ok(request) => {
                self.selected_path = Some(request.path.clone());
                collection.requests.push(request);
                collection
                    .requests
                    .sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
                self.load_selected_draft(window, cx);
                self.status_message = Some("Created request".to_string());
            }
            Err(error) => self.status_message = Some(error.to_string()),
        }
        cx.notify();
    }

    fn send_selected(&mut self, cx: &mut Context<Self>) {
        self.is_sending = true;
        self.response = None;
        self.response_body_pretty = true;
        self.status_message = Some("Sending request".to_string());
        let client = self.http_client.clone();
        let draft = self.draft.clone();

        cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move { http_client::send_request(client, draft) })
                .await;

            let _ = this.update(cx, |this, cx| {
                this.is_sending = false;
                match result {
                    Ok(response) => {
                        this.status_message = Some(format!(
                            "{} {} in {} ms",
                            response.status,
                            response.status_text,
                            response.duration_ms()
                        ));
                        this.response = Some(response);
                    }
                    Err(error) => {
                        this.status_message = Some(error.to_string());
                    }
                }
                cx.notify();
            });
        })
        .detach();

        cx.notify();
    }

    fn on_url_input(
        this: &mut Self,
        state: &Entity<InputState>,
        event: &InputEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if matches!(event, InputEvent::Change) {
            this.draft.url = state.read(cx).value().to_string();
            this.is_dirty = true;
            cx.notify();
        }
    }

    fn on_method_select(
        this: &mut Self,
        _: &Entity<SelectState<Vec<SharedString>>>,
        event: &SelectEvent<Vec<SharedString>>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let SelectEvent::Confirm(Some(value)) = event
            && let Ok(method) = value.as_ref().parse::<HttpMethod>()
        {
            this.draft.method = method;
            this.is_dirty = true;
            cx.notify();
        }
    }

    fn on_headers_input(
        this: &mut Self,
        state: &Entity<InputState>,
        event: &InputEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if matches!(event, InputEvent::Change) {
            this.draft.headers = parse_rows(&state.read(cx).value());
            this.is_dirty = true;
            cx.notify();
        }
    }

    fn on_params_input(
        this: &mut Self,
        state: &Entity<InputState>,
        event: &InputEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if matches!(event, InputEvent::Change) {
            this.draft.params = parse_rows(&state.read(cx).value());
            this.is_dirty = true;
            cx.notify();
        }
    }

    fn on_body_input(
        this: &mut Self,
        state: &Entity<InputState>,
        event: &InputEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if matches!(event, InputEvent::Change) {
            this.draft.body = BodyMode::from_editor_text(state.read(cx).value().to_string());
            this.is_dirty = true;
            cx.notify();
        }
    }

    fn sync_inputs_from_draft(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.method_select.update(cx, |state, cx| {
            state.set_selected_value(&SharedString::from(self.draft.method.as_str()), window, cx);
        });
        self.url_input.update(cx, |state, cx| {
            state.set_value(self.draft.url.clone(), window, cx);
        });
        self.headers_input.update(cx, |state, cx| {
            state.set_value(format_rows(&self.draft.headers), window, cx);
        });
        self.params_input.update(cx, |state, cx| {
            state.set_value(format_rows(&self.draft.params), window, cx);
        });
        self.body_input.update(cx, |state, cx| {
            state.set_highlighter(self.draft.body.language(), cx);
            state.set_value(self.draft.body.text().to_string(), window, cx);
        });
    }

    fn render_sidebar(&self, cx: &mut Context<Self>) -> impl gpui::IntoElement {
        let collection_name = self
            .collection
            .as_ref()
            .map(|collection| collection.name.clone())
            .unwrap_or_else(|| "No collection".to_string());

        v_flex()
            .w(px(280.))
            .h_full()
            .border_r_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().sidebar)
            .child(
                h_flex()
                    .justify_between()
                    .p_3()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w(px(0.))
                            .child(div().font_semibold().child(collection_name)),
                    )
                    .child(
                        Button::new("new-request")
                            .ghost()
                            .flex_shrink_0()
                            .small()
                            .icon(IconName::Plus)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.create_request(window, cx);
                            })),
                    ),
            )
            .child(v_flex().flex_1().overflow_y_scrollbar().p_2().children(
                self.collection.as_ref().into_iter().flat_map(|collection| {
                    collection.requests.iter().map(|request| {
                        let selected = Some(&request.path) == self.selected_path.as_ref();
                        let path = request.path.clone();
                        let method = request.draft.method.to_string();

                        h_flex()
                            .id(format!("request-{}", request.relative_path.display()))
                            .px_2()
                            .py_1()
                            .justify_between()
                            .rounded_sm()
                            .when(selected, |el| el.bg(cx.theme().button_active))
                            .hover(|style| style.bg(cx.theme().button_hover))
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.select_request(path.clone(), window, cx);
                            }))
                            .child(request.draft.name.clone())
                            .child(
                                Label::new(method).text_color(method_color(&request.draft.method)),
                            )
                    })
                }),
            ))
    }

    fn render_request_editor(&self, cx: &mut Context<Self>) -> impl gpui::IntoElement {
        let save_target = cx.entity().downgrade();

        v_flex()
            .flex_1()
            .flex_basis(relative(0.))
            .min_h(px(0.))
            .border_b_1()
            .border_color(cx.theme().border)
            .child(
                h_flex()
                    .gap_2()
                    .p_3()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(
                        h_flex()
                            .flex_1()
                            .border_1()
                            .rounded(cx.theme().radius)
                            .border_color(cx.theme().input)
                            .text_color(cx.theme().secondary_foreground)
                            .bg(cx.theme().background)
                            .child(
                                div().w(px(140.)).child(
                                    Select::new(&self.method_select)
                                        .appearance(false)
                                        .placeholder("Method"),
                                ),
                            )
                            .child(Separator::vertical())
                            .child(
                                div()
                                    .flex_1()
                                    .child(Input::new(&self.url_input).appearance(false)),
                            )
                            .child(Separator::vertical())
                            .child(
                                div().child(
                                    DropdownButton::new("send-request-menu")
                                        .small()
                                        .ghost()
                                        .button(Button::new("send-request").label("Send").on_click(
                                            cx.listener(|this, _, _, cx| {
                                                this.send_selected(cx);
                                            }),
                                        ))
                                        .loading(self.is_sending)
                                        .dropdown_menu(move |menu, _, _| {
                                            let save_target = save_target.clone();
                                            menu.label("Request")
                                                .separator()
                                                .item(PopupMenuItem::new("Save").on_click(
                                                    move |_, _, cx| {
                                                        let _ =
                                                            save_target.update(cx, |this, cx| {
                                                                if let Err(error) =
                                                                    this.save_selected()
                                                                {
                                                                    this.status_message =
                                                                        Some(error.to_string());
                                                                }
                                                                cx.notify();
                                                            });
                                                    },
                                                ))
                                                .item(PopupMenuItem::new("Save as").disabled(true))
                                                .item(
                                                    PopupMenuItem::new("Duplicate request")
                                                        .disabled(true),
                                                )
                                        }),
                                ),
                            ),
                    ),
            )
            .child(
                v_flex()
                    .flex_1()
                    .min_h(px(0.))
                    .p_3()
                    .gap_3()
                    .child(
                        TabBar::new("request-tabs")
                            .segmented()
                            .selected_index(self.request_tab.index())
                            .on_click(cx.listener(|this, index, _, cx| {
                                this.request_tab = match *index {
                                    1 => RequestTab::Params,
                                    2 => RequestTab::Body,
                                    _ => RequestTab::Headers,
                                };
                                cx.notify();
                            }))
                            .child(Tab::new().label("Headers"))
                            .child(Tab::new().label("Params"))
                            .child(Tab::new().label("Body")),
                    )
                    .child(match self.request_tab {
                        RequestTab::Headers => self.render_editor_textarea(
                            "Headers",
                            "One header per line, written as Name: value",
                            &self.headers_input,
                            cx,
                        ),
                        RequestTab::Params => self.render_editor_textarea(
                            "Params",
                            "One query param per line, written as name: value",
                            &self.params_input,
                            cx,
                        ),
                        RequestTab::Body => self.render_editor_textarea(
                            "Body",
                            "JSON is detected automatically, otherwise raw text is sent",
                            &self.body_input,
                            cx,
                        ),
                    }),
            )
    }

    fn render_editor_textarea(
        &self,
        title: &'static str,
        help: &'static str,
        input: &Entity<InputState>,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        v_flex()
            .flex_1()
            .min_h(px(0.))
            .gap_2()
            .child(
                h_flex()
                    .justify_between()
                    .child(div().font_medium().child(title))
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(help),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .min_h(px(0.))
                    .child(Input::new(input).appearance(false).h_full()),
            )
            .into_any_element()
    }

    fn render_response(&self, cx: &mut Context<Self>) -> impl gpui::IntoElement {
        let status = self.status_message.clone().unwrap_or_else(|| {
            "OpenCollection v1: edit a request, save it to YAML, then send it".to_string()
        });

        v_flex()
            .flex_1()
            .flex_basis(relative(0.))
            .min_h(px(0.))
            .child(
                h_flex()
                    .justify_between()
                    .p_3()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(
                        TabBar::new("response-tabs")
                            .segmented()
                            .selected_index(self.response_tab.index())
                            .on_click(cx.listener(|this, index, _, cx| {
                                this.response_tab = match *index {
                                    1 => ResponseTab::Headers,
                                    2 => ResponseTab::Meta,
                                    _ => ResponseTab::Body,
                                };
                                cx.notify();
                            }))
                            .child(Tab::new().label("Body"))
                            .child(Tab::new().label("Headers"))
                            .child(Tab::new().label("Meta")),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child(status),
                    ),
            )
            .child(div().flex_1().child(div().size_full().p_3().child(
                match (&self.response, self.response_tab) {
                    (Some(response), ResponseTab::Body) => self.render_response_body(response, cx),
                    (Some(response), ResponseTab::Headers) => {
                        self.render_response_headers(response, cx)
                    }
                    (Some(response), ResponseTab::Meta) => self.render_response_meta(response, cx),
                    (None, _) => code_block("No response yet.".to_string(), cx),
                },
            )))
    }

    fn render_response_body(
        &self,
        response: &ResponseRecord,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let text = if self.response_body_pretty {
            response.pretty_body.clone()
        } else {
            response.body.clone()
        };
        let visible_text = if text.is_empty() {
            "No response body.".to_string()
        } else {
            text.clone()
        };
        let mode = if self.response_body_pretty {
            "Pretty"
        } else {
            "Raw"
        };

        v_flex()
            .flex_1()
            .h_full()
            .gap_2()
            .child(
                h_flex()
                    .flex_shrink_0()
                    .justify_between()
                    .child(
                        h_flex()
                            .gap_2()
                            .child(
                                ButtonGroup::new("response-body-mode")
                                    .small()
                                    .child(
                                        Button::new("response-body-pretty")
                                            .label("Pretty")
                                            .selected(self.response_body_pretty),
                                    )
                                    .child(
                                        Button::new("response-body-raw")
                                            .label("Raw")
                                            .selected(!self.response_body_pretty),
                                    )
                                    .on_click(cx.listener(|this, selected: &Vec<usize>, _, cx| {
                                        this.response_body_pretty = !selected.contains(&1);
                                        cx.notify();
                                    })),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(format!("{} body, {} bytes", mode, response.size_bytes)),
                            ),
                    )
                    .child(
                        Clipboard::new("copy-response-body")
                            .tooltip("Copy response body")
                            .value(text.clone()),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .h_full()
                    .min_h(px(0.))
                    .overflow_y_scrollbar()
                    .child(code_block(visible_text, cx)),
            )
            .into_any_element()
    }

    fn render_response_headers(
        &self,
        response: &ResponseRecord,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let header_text = format_rows(&response.headers);

        v_flex()
            .flex_1()
            .h_full()
            .min_h(px(0.))
            .overflow_hidden()
            .gap_2()
            .child(
                h_flex()
                    .flex_shrink_0()
                    .justify_between()
                    .child(
                        div()
                            .text_sm()
                            .font_medium()
                            .child(format!("{} headers", response.headers.len())),
                    )
                    .child(
                        Clipboard::new("copy-response-headers")
                            .tooltip("Copy response headers")
                            .value(header_text),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .h_full()
                    .min_h(px(0.))
                    .overflow_y_scrollbar()
                    .child(
                        v_flex()
                            .rounded(cx.theme().radius)
                            .border_1()
                            .border_color(cx.theme().border)
                            .bg(cx.theme().muted)
                            .children(response.headers.iter().enumerate().map(|(index, row)| {
                                h_flex()
                                    .min_h(px(34.))
                                    .items_start()
                                    .gap_3()
                                    .px_3()
                                    .py_2()
                                    .when(index + 1 < response.headers.len(), |el| {
                                        el.border_b_1().border_color(cx.theme().border)
                                    })
                                    .child(
                                        div()
                                            .w(px(220.))
                                            .flex_shrink_0()
                                            .font_family(JETBRAINS_MONO)
                                            .text_sm()
                                            .text_color(cx.theme().muted_foreground)
                                            .child(row.key.clone()),
                                    )
                                    .child(
                                        div()
                                            .flex_1()
                                            .min_w(px(0.))
                                            .whitespace_normal()
                                            .font_family(JETBRAINS_MONO)
                                            .text_sm()
                                            .child(row.value.clone()),
                                    )
                            })),
                    ),
            )
            .into_any_element()
    }

    fn render_response_meta(
        &self,
        response: &ResponseRecord,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let rows = [
            (
                "Status",
                format!("{} {}", response.status, response.status_text),
            ),
            ("Duration", format!("{} ms", response.duration_ms())),
            ("Size", format!("{} bytes", response.size_bytes)),
            (
                "Decoded body",
                format!("{} UTF-8 characters", response.body.chars().count()),
            ),
        ];
        let row_count = rows.len();

        v_flex()
            .rounded(cx.theme().radius)
            .border_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().muted)
            .children(rows.into_iter().enumerate().map(|(index, (label, value))| {
                h_flex()
                    .min_h(px(34.))
                    .gap_3()
                    .px_3()
                    .py_2()
                    .when(index + 1 < row_count, |el| {
                        el.border_b_1().border_color(cx.theme().border)
                    })
                    .child(
                        div()
                            .w(px(140.))
                            .flex_shrink_0()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child(label),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .font_family(JETBRAINS_MONO)
                            .text_sm()
                            .child(value),
                    )
            }))
            .into_any_element()
    }
}

impl Render for WorkspaceScreen {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl gpui::IntoElement {
        h_flex()
            .size_full()
            .items_stretch()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(self.render_sidebar(cx))
            .child(
                v_flex()
                    .flex_1()
                    .h_full()
                    .min_h(px(0.))
                    .min_w(px(480.))
                    .child(self.render_request_editor(cx))
                    .child(self.render_response(cx)),
            )
    }
}

fn code_block(text: String, cx: &mut Context<WorkspaceScreen>) -> gpui::AnyElement {
    div()
        .w_full()
        .min_h(px(140.))
        .p_3()
        .rounded(cx.theme().radius)
        .bg(cx.theme().muted)
        .text_sm()
        .font_family(JETBRAINS_MONO)
        .child(text)
        .into_any_element()
}

fn method_color(method: &HttpMethod) -> Hsla {
    match method {
        HttpMethod::Get => green_300(),
        HttpMethod::Post => orange_300(),
        HttpMethod::Put => orange_300(),
        HttpMethod::Patch => orange_300(),
        HttpMethod::Delete => red_300(),
        HttpMethod::Head => blue_300(),
        HttpMethod::Options => blue_300(),
    }
}

fn parse_rows(value: &SharedString) -> Vec<KeyValueRow> {
    value
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() {
                return None;
            }
            let (key, value) = line.split_once(':').unwrap_or((line, ""));
            Some(KeyValueRow {
                enabled: true,
                key: key.trim().to_string(),
                value: value.trim().to_string(),
            })
        })
        .collect()
}

fn format_rows(rows: &[KeyValueRow]) -> String {
    rows.iter()
        .filter(|row| row.enabled && !row.key.trim().is_empty())
        .map(|row| format!("{}: {}", row.key, row.value))
        .collect::<Vec<_>>()
        .join("\n")
}

fn default_collection_path() -> PathBuf {
    std::env::var_os("ZACK_COLLECTION")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("sample-collection"))
}

#[allow(dead_code)]
fn _duration_for_tests() -> Duration {
    Duration::from_millis(0)
}
