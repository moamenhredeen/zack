use std::path::PathBuf;

use anyhow::{Result, anyhow};
use gpui::{
    AppContext, Context, Entity, Hsla, InteractiveElement, IntoElement, ParentElement,
    PathPromptOptions, Render, SharedString, StatefulInteractiveElement, Styled, Window, div,
    prelude::FluentBuilder, px, relative,
};
use gpui_component::{
    ActiveTheme, IconName, Selectable, Sizable, StyledExt, blue_300,
    button::{Button, ButtonGroup, ButtonVariants, DropdownButton},
    clipboard::Clipboard,
    green_300, h_flex,
    input::Input,
    label::Label,
    menu::PopupMenuItem,
    orange_300, red_300,
    scroll::ScrollableElement,
    select::Select,
    separator::Separator,
    tab::{Tab, TabBar},
    v_flex,
};

use opencollection::{HttpRequest, HttpResponseHeader};

use crate::{
    collection_store::{CollectionEvent, CollectionStore, ItemPath},
    fonts::JETBRAINS_MONO,
    http_client,
    model::ResponseRecord,
    request_tab::{RequestPane, RequestTab, ResponsePane, TabDirtied, TabTarget, join_lines},
};

gpui::actions!(zack, [SaveTab, SaveAll]);

/// The key context the save bindings are scoped to.
///
/// gpui dispatches actions up the focus chain, so this name has to appear on
/// the workspace's root element for the bindings in `main` to reach it.
pub const KEY_CONTEXT: &str = "Workspace";

pub struct WorkspaceScreen {
    focus_handle: gpui::FocusHandle,
    collections: Entity<CollectionStore>,
    /// The open tabs, each holding its own draft.
    ///
    /// Because a tab owns the editors it is being edited in, switching tabs —
    /// or collections — no longer has to flush anything to disk: an unsaved
    /// draft simply stays in the tab it belongs to.
    tabs: Vec<Entity<RequestTab>>,
    active: Option<usize>,
    status_message: Option<String>,
    http_client: reqwest::blocking::Client,
}

impl WorkspaceScreen {
    pub fn new(
        collections: Entity<CollectionStore>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.subscribe_in(&collections, window, Self::on_collection_event)
            .detach();

        let mut this = Self {
            focus_handle: cx.focus_handle(),
            collections,
            tabs: Vec::new(),
            active: None,
            status_message: None,
            http_client: reqwest::blocking::Client::new(),
        };

        if let Some(target) = this.selected_target(cx) {
            this.open_tab(target, window, cx);
        }
        this
    }

    fn active_tab(&self) -> Option<&Entity<RequestTab>> {
        self.tabs.get(self.active?)
    }

    /// The request highlighted in the sidebar, as a tab target.
    fn selected_target(&self, cx: &mut Context<Self>) -> Option<TabTarget> {
        let store = self.collections.read(cx);
        Some(TabTarget {
            collection: store.active_index(),
            item: store.active().selected.clone()?,
        })
    }

    /// Focuses the tab editing `target`, opening one if it is not already open.
    fn open_tab(&mut self, target: TabTarget, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(index) = self
            .tabs
            .iter()
            .position(|tab| tab.read(cx).target == target)
        {
            self.activate_tab(index, cx);
            return;
        }

        let Some((title, saved)) = self
            .collections
            .read(cx)
            .collection(target.collection)
            .and_then(|collection| collection.request_at(&target.item))
            .map(|request| {
                (
                    request_name(request),
                    request.http.clone().unwrap_or_default(),
                )
            })
        else {
            return;
        };

        // Without focus somewhere inside the workspace, the save bindings have
        // no dispatch path and Cmd-S does nothing until the user clicks a field.
        window.focus(&self.focus_handle, cx);

        let tab = cx.new(|cx| RequestTab::new(target, title, saved, window, cx));
        // A tab going dirty changes the tab bar, which the workspace draws.
        cx.subscribe(&tab, |_, _, _: &TabDirtied, cx| cx.notify())
            .detach();
        cx.observe(&tab, |_, _, cx| cx.notify()).detach();

        self.tabs.push(tab);
        self.activate_tab(self.tabs.len() - 1, cx);
    }

    /// Makes `index` the visible tab, following the sidebar to its collection.
    fn activate_tab(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(tab) = self.tabs.get(index) else {
            return;
        };
        self.active = Some(index);

        let target = tab.read(cx).target.clone();
        self.collections.update(cx, |store, _| {
            store.set_active(target.collection);
            store.active_mut().selected = Some(target.item);
        });
        cx.notify();
    }

    fn close_tab(&mut self, index: usize, cx: &mut Context<Self>) {
        if index >= self.tabs.len() {
            return;
        }

        // Drafts live only in the tab, so closing one throws the edits away.
        // Saying so is the least that is owed until there is a real prompt.
        if self.tabs[index].read(cx).is_dirty {
            self.status_message = Some("Closed tab — unsaved changes discarded".to_string());
        }
        self.tabs.remove(index);

        self.active = match self.active {
            _ if self.tabs.is_empty() => None,
            Some(active) if active > index => Some(active - 1),
            Some(active) if active == index => Some(index.min(self.tabs.len() - 1)),
            other => other,
        };
        if let Some(active) = self.active {
            self.activate_tab(active, cx);
        }
        cx.notify();
    }

    /// Writes the active tab's draft into the document and out to its own file.
    fn save_active_tab(&mut self, cx: &mut Context<Self>) -> Result<()> {
        let Some(index) = self.active else {
            return Err(anyhow!("no request open"));
        };
        self.save_tab(index, cx)?;
        self.status_message = Some("Saved request".to_string());
        Ok(())
    }

    /// Writes one tab's draft into the document and out to its own file.
    ///
    /// Only the tab's own file is touched, so saving one tab cannot disturb the
    /// unsaved drafts sitting in the others.
    fn save_tab(&mut self, index: usize, cx: &mut Context<Self>) -> Result<()> {
        let Some(tab) = self.tabs.get(index).cloned() else {
            return Err(anyhow!("no request open"));
        };

        let target = tab.read(cx).target.clone();
        let draft = tab.read(cx).draft(cx);

        let saved = draft.clone();
        self.collections.update(cx, |store, _| {
            let collection = store
                .collection_mut(target.collection)
                .ok_or_else(|| anyhow!("this collection is no longer open"))?;
            let request = collection
                .request_at_mut(&target.item)
                .ok_or_else(|| anyhow!("the request this tab was editing is gone"))?;
            request.http = Some(saved);
            collection.save_item(&target.item)
        })?;

        tab.update(cx, |tab, cx| tab.mark_saved(draft, cx));
        Ok(())
    }

    /// Saves every tab that has unsaved changes, in tab order.
    ///
    /// Stops at the first failure rather than pressing on, so a collection that
    /// has gone missing under us reports once, naming the request, instead of
    /// once per tab.
    fn save_all_tabs(&mut self, cx: &mut Context<Self>) -> Result<()> {
        let dirty: Vec<usize> = self
            .tabs
            .iter()
            .enumerate()
            .filter(|(_, tab)| tab.read(cx).is_dirty)
            .map(|(index, _)| index)
            .collect();

        if dirty.is_empty() {
            self.status_message = Some("No unsaved changes".to_string());
            return Ok(());
        }

        let saved = dirty.len();
        for index in dirty {
            let title = self.tabs[index].read(cx).title.clone();
            self.save_tab(index, cx)
                .map_err(|error| anyhow!("{title}: {error}"))?;
        }

        self.status_message = Some(match saved {
            1 => "Saved 1 request".to_string(),
            n => format!("Saved {n} requests"),
        });
        Ok(())
    }

    /// Throws the active tab's draft away and shows the request as saved.
    fn revert_active_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(tab) = self.active_tab().cloned() else {
            return;
        };
        let target = tab.read(cx).target.clone();
        let Some(saved) = self
            .collections
            .read(cx)
            .collection(target.collection)
            .and_then(|collection| collection.request_at(&target.item))
            .map(|request| request.http.clone().unwrap_or_default())
        else {
            return;
        };

        tab.update(cx, |tab, cx| tab.reset_to(saved, window, cx));
        self.status_message = Some("Reverted to last saved".to_string());
        cx.notify();
    }

    fn on_collection_event(
        &mut self,
        _: &Entity<CollectionStore>,
        _: &CollectionEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        cx.notify();
    }

    fn switch_collection(&mut self, index: usize, cx: &mut Context<Self>) {
        self.collections.update(cx, |store, cx| {
            store.set_active(index);
            cx.emit(CollectionEvent::ActiveChanged);
        });
        // Tabs hold their own drafts, so nothing needs flushing here; the tab
        // bar simply keeps showing every open request.
        cx.notify();
    }

    /// Asks for a location, then creates a collection directory there.
    fn create_collection(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let receiver = cx.prompt_for_new_path(&picker_start_dir(), Some("my-collection"));
        cx.spawn_in(window, async move |this, cx| {
            let Ok(Ok(Some(root))) = receiver.await else {
                return;
            };
            let _ = this.update_in(cx, |this, window, cx| {
                let result = this.collections.update(cx, |store, _| store.create(&root));
                this.after_collection_change(result, "Created collection", window, cx);
            });
        })
        .detach();
    }

    /// Asks for an existing collection directory and opens it.
    fn open_collection(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let receiver = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("Open".into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            let Ok(Ok(Some(roots))) = receiver.await else {
                return;
            };
            let Some(root) = roots.into_iter().next() else {
                return;
            };
            let _ = this.update_in(cx, |this, window, cx| {
                let result = this.collections.update(cx, |store, _| store.open(&root));
                this.after_collection_change(result, "Opened collection", window, cx);
            });
        })
        .detach();
    }

    fn after_collection_change(
        &mut self,
        result: Result<()>,
        success_message: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match result {
            Ok(()) => {
                if let Some(target) = self.selected_target(cx) {
                    self.open_tab(target, window, cx);
                }
                self.status_message = Some(success_message.to_string());
            }
            Err(error) => self.status_message = Some(error.to_string()),
        }
        cx.notify();
    }

    fn select_request(&mut self, path: ItemPath, window: &mut Window, cx: &mut Context<Self>) {
        let collection = self.collections.read(cx).active_index();
        self.open_tab(
            TabTarget {
                collection,
                item: path,
            },
            window,
            cx,
        );
    }

    fn create_request(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.collections.update(cx, |store, _| {
            store.active_mut().create_request("New request");
        });
        if let Some(target) = self.selected_target(cx) {
            self.open_tab(target, window, cx);
        }
        self.status_message = Some("Created request".to_string());
        cx.notify();
    }

    /// Sends what the user is looking at — the draft, not the saved request.
    fn send_active_tab(&mut self, cx: &mut Context<Self>) {
        let Some(tab) = self.active_tab().cloned() else {
            self.status_message = Some("No request open".to_string());
            cx.notify();
            return;
        };

        let target = tab.read(cx).target.clone();
        let draft = tab.read(cx).draft(cx);
        let mut request = self
            .collections
            .read(cx)
            .collection(target.collection)
            .and_then(|collection| collection.request_at(&target.item))
            .cloned()
            .unwrap_or_else(|| HttpRequest::get(""));
        request.http = Some(draft);

        tab.update(cx, |tab, cx| {
            tab.is_sending = true;
            tab.response = None;
            tab.response_body_pretty = true;
            cx.notify();
        });
        self.status_message = Some("Sending request".to_string());
        let client = self.http_client.clone();

        cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move { http_client::send_request(client, request) })
                .await;

            let _ = this.update(cx, |this, cx| {
                let message = match &result {
                    Ok(response) => format!(
                        "{} {} in {} ms",
                        response.status,
                        response.status_text,
                        response.duration_ms()
                    ),
                    Err(error) => error.to_string(),
                };
                tab.update(cx, |tab, cx| {
                    tab.is_sending = false;
                    tab.response = result.ok();
                    cx.notify();
                });
                this.status_message = Some(message);
                cx.notify();
            });
        })
        .detach();

        cx.notify();
    }

    fn render_tab_bar(&self, cx: &mut Context<Self>) -> impl gpui::IntoElement {
        let tabs = self
            .tabs
            .iter()
            .enumerate()
            .map(|(index, tab)| {
                let tab = tab.read(cx);
                (index, tab.title.clone(), tab.is_dirty)
            })
            .collect::<Vec<_>>();

        h_flex()
            .w_full()
            .border_b_1()
            .border_color(cx.theme().border)
            .child(
                TabBar::new("request-tab-bar")
                    .w_full()
                    .selected_index(self.active.unwrap_or(0))
                    .on_click(cx.listener(|this, index: &usize, _, cx| {
                        this.activate_tab(*index, cx);
                    }))
                    .children(tabs.into_iter().map(|(index, title, is_dirty)| {
                        Tab::new()
                            // Tab only pads its label; the prefix and suffix sit
                            // outside that, flush against the tab's own borders.
                            // Padding the tab itself covers all three.
                            .px_2()
                            .label(title)
                            // A leading dot marks the unsaved draft, the way an
                            // editor marks a modified buffer. It goes before the
                            // label so a long request name cannot push it out of
                            // view, and the close button stays put either way —
                            // a dirty tab still has to be closable.
                            .when(is_dirty, |tab| {
                                tab.prefix(div().text_color(orange_300()).child("●"))
                            })
                            .suffix(
                                Button::new(SharedString::from(format!("close-tab-{index}")))
                                    .ghost()
                                    .xsmall()
                                    .icon(IconName::Close)
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.close_tab(index, cx);
                                    })),
                            )
                    })),
            )
    }

    fn render_sidebar(&self, cx: &mut Context<Self>) -> impl gpui::IntoElement {
        let store = self.collections.read(cx);
        let active_index = store.active_index();
        let collection_name = store.active().name();
        let switcher_items = store
            .collections()
            .iter()
            .enumerate()
            .map(|(index, collection)| (index, collection.name()))
            .collect::<Vec<_>>();
        let requests = store
            .active()
            .requests()
            .into_iter()
            .map(|(path, request)| {
                let method = request
                    .http
                    .as_ref()
                    .and_then(|details| details.method.clone())
                    .unwrap_or_else(|| "GET".to_string());
                (path, request_name(request), method)
            })
            .collect::<Vec<_>>();
        let selected_path = store.active().selected.clone();

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
                        v_flex().flex_1().min_w(px(0.)).child(
                            DropdownButton::new("collection-switcher")
                                .small()
                                .ghost()
                                .button(
                                    Button::new("collection-name")
                                        .label(collection_name)
                                        .font_semibold(),
                                )
                                .dropdown_menu({
                                    let switch_target = cx.entity();
                                    move |mut menu, _, _| {
                                        menu = menu.label("Collections").separator();
                                        for (index, name) in switcher_items.clone() {
                                            let switch_target = switch_target.clone();
                                            menu = menu.item(
                                                PopupMenuItem::new(name)
                                                    .checked(index == active_index)
                                                    .on_click(move |_, _, cx| {
                                                        switch_target.update(cx, |this, cx| {
                                                            this.switch_collection(index, cx);
                                                        });
                                                    }),
                                            );
                                        }
                                        menu.separator()
                                            .item(PopupMenuItem::new("New collection…").on_click({
                                                let target = switch_target.clone();
                                                move |_, window, cx| {
                                                    target.update(cx, |this, cx| {
                                                        this.create_collection(window, cx);
                                                    });
                                                }
                                            }))
                                            .item(PopupMenuItem::new("Open collection…").on_click(
                                                {
                                                    let target = switch_target.clone();
                                                    move |_, window, cx| {
                                                        target.update(cx, |this, cx| {
                                                            this.open_collection(window, cx);
                                                        });
                                                    }
                                                },
                                            ))
                                    }
                                }),
                        ),
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
                requests.into_iter().map(|(path, name, method)| {
                    let selected = Some(&path) == selected_path.as_ref();

                    h_flex()
                        .id(SharedString::from(format!("request-{path:?}")))
                        .px_2()
                        .py_1()
                        .justify_between()
                        .rounded_sm()
                        .when(selected, |el| el.bg(cx.theme().button_active))
                        .hover(|style| style.bg(cx.theme().button_hover))
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.select_request(path.clone(), window, cx);
                        }))
                        .child(name)
                        .child(Label::new(method.clone()).text_color(method_color(&method)))
                }),
            ))
    }

    fn render_request_editor(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let Some(tab) = self.active_tab().cloned() else {
            return v_flex()
                .flex_1()
                .flex_basis(relative(0.))
                .min_h(px(0.))
                .items_center()
                .justify_center()
                .border_b_1()
                .border_color(cx.theme().border)
                .text_color(cx.theme().muted_foreground)
                .child("Select a request to start editing")
                .into_any_element();
        };

        // Cloned out rather than borrowed: rendering the panes needs `cx`
        // mutably, and the entity handles are cheap to clone.
        let save_target = cx.entity().downgrade();
        let state = tab.read(cx);
        let pane = state.pane;
        let is_sending = state.is_sending;
        let method_select = state.method_select.clone();
        let url_input = state.url_input.clone();
        let headers_input = state.headers_input.clone();
        let params_input = state.params_input.clone();
        let body_input = state.body_input.clone();

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
                                    Select::new(&method_select)
                                        .appearance(false)
                                        .placeholder("Method"),
                                ),
                            )
                            .child(Separator::vertical())
                            .child(
                                div()
                                    .flex_1()
                                    .child(Input::new(&url_input).appearance(false)),
                            )
                            .child(Separator::vertical())
                            .child(
                                div().child(
                                    DropdownButton::new("send-request-menu")
                                        .small()
                                        .ghost()
                                        .button(Button::new("send-request").label("Send").on_click(
                                            cx.listener(|this, _, _, cx| {
                                                this.send_active_tab(cx);
                                            }),
                                        ))
                                        .loading(is_sending)
                                        .dropdown_menu(move |menu, _, _| {
                                            let save_target = save_target.clone();
                                            let revert_target = save_target.clone();
                                            menu.label("Request")
                                                .separator()
                                                .item(PopupMenuItem::new("Save").on_click(
                                                    move |_, _, cx| {
                                                        let _ =
                                                            save_target.update(cx, |this, cx| {
                                                                if let Err(error) =
                                                                    this.save_active_tab(cx)
                                                                {
                                                                    this.status_message =
                                                                        Some(error.to_string());
                                                                }
                                                                cx.notify();
                                                            });
                                                    },
                                                ))
                                                .item(
                                                    PopupMenuItem::new("Revert to saved").on_click(
                                                        move |_, window, cx| {
                                                            let _ = revert_target.update(
                                                                cx,
                                                                |this, cx| {
                                                                    this.revert_active_tab(
                                                                        window, cx,
                                                                    );
                                                                },
                                                            );
                                                        },
                                                    ),
                                                )
                                                .item(PopupMenuItem::new("Save as").disabled(true))
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
                        TabBar::new("request-panes")
                            .segmented()
                            .selected_index(pane.index())
                            .on_click(cx.listener(|this, index: &usize, _, cx| {
                                let pane = RequestPane::from_index(*index);
                                if let Some(tab) = this.active_tab().cloned() {
                                    tab.update(cx, |tab, cx| {
                                        tab.pane = pane;
                                        cx.notify();
                                    });
                                }
                            }))
                            .child(Tab::new().label("Headers"))
                            .child(Tab::new().label("Params"))
                            .child(Tab::new().label("Body")),
                    )
                    .child(match pane {
                        RequestPane::Headers => self.render_editor_textarea(
                            "Headers",
                            "One header per line, written as Name: value",
                            &headers_input,
                            cx,
                        ),
                        RequestPane::Params => self.render_editor_textarea(
                            "Params",
                            "One query param per line, written as name: value",
                            &params_input,
                            cx,
                        ),
                        RequestPane::Body => self.render_editor_textarea(
                            "Body",
                            "JSON is detected automatically, otherwise raw text is sent",
                            &body_input,
                            cx,
                        ),
                    }),
            )
            .into_any_element()
    }

    fn render_editor_textarea(
        &self,
        title: &'static str,
        help: &'static str,
        input: &Entity<gpui_component::input::InputState>,
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
        let tab = self.active_tab().cloned();
        let pane = tab
            .as_ref()
            .map_or(ResponsePane::Body, |tab| tab.read(cx).response_pane);
        let response = tab.as_ref().and_then(|tab| tab.read(cx).response.clone());

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
                        TabBar::new("response-panes")
                            .segmented()
                            .selected_index(pane.index())
                            .on_click(cx.listener(|this, index: &usize, _, cx| {
                                let pane = ResponsePane::from_index(*index);
                                if let Some(tab) = this.active_tab().cloned() {
                                    tab.update(cx, |tab, cx| {
                                        tab.response_pane = pane;
                                        cx.notify();
                                    });
                                }
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
            .child(
                div()
                    .flex_1()
                    .child(div().size_full().p_3().child(match (&response, pane) {
                        (Some(response), ResponsePane::Body) => {
                            self.render_response_body(response, cx)
                        }
                        (Some(response), ResponsePane::Headers) => {
                            self.render_response_headers(response, cx)
                        }
                        (Some(response), ResponsePane::Meta) => {
                            self.render_response_meta(response, cx)
                        }
                        (None, _) => code_block("No response yet.".to_string(), cx),
                    })),
            )
    }

    fn render_response_body(
        &self,
        response: &ResponseRecord,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let pretty = self
            .active_tab()
            .is_none_or(|tab| tab.read(cx).response_body_pretty);
        let text = if pretty {
            response.pretty_body.clone()
        } else {
            response.body.clone()
        };
        let visible_text = if text.is_empty() {
            "No response body.".to_string()
        } else {
            text.clone()
        };
        let mode = if pretty { "Pretty" } else { "Raw" };

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
                                            .selected(pretty),
                                    )
                                    .child(
                                        Button::new("response-body-raw")
                                            .label("Raw")
                                            .selected(!pretty),
                                    )
                                    .on_click(cx.listener(|this, selected: &Vec<usize>, _, cx| {
                                        let pretty = !selected.contains(&1);
                                        if let Some(tab) = this.active_tab().cloned() {
                                            tab.update(cx, |tab, cx| {
                                                tab.response_body_pretty = pretty;
                                                cx.notify();
                                            });
                                        }
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
        let header_text = format_response_headers(&response.headers);

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
                                            .child(row.name.clone()),
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
            .track_focus(&self.focus_handle)
            .key_context(KEY_CONTEXT)
            .on_action(cx.listener(|this, _: &SaveTab, _, cx| {
                if let Err(error) = this.save_active_tab(cx) {
                    this.status_message = Some(error.to_string());
                }
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &SaveAll, _, cx| {
                if let Err(error) = this.save_all_tabs(cx) {
                    this.status_message = Some(error.to_string());
                }
                cx.notify();
            }))
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
                    .child(self.render_tab_bar(cx))
                    .child(self.render_request_editor(cx))
                    .child(self.render_response(cx)),
            )
    }
}

fn request_name(request: &HttpRequest) -> String {
    request
        .info
        .as_ref()
        .and_then(|info| info.name.clone())
        .unwrap_or_else(|| "Untitled".to_string())
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

fn method_color(method: &str) -> Hsla {
    match method.to_ascii_uppercase().as_str() {
        "GET" => green_300(),
        "POST" | "PUT" | "PATCH" => orange_300(),
        "DELETE" => red_300(),
        _ => blue_300(),
    }
}

fn format_response_headers(headers: &[HttpResponseHeader]) -> String {
    join_lines(headers.iter().map(|header| (&header.name, &header.value)))
}

/// Where the native file picker opens by default.
fn picker_start_dir() -> PathBuf {
    dirs::document_dir()
        .or_else(dirs::home_dir)
        .unwrap_or_else(|| PathBuf::from("."))
}
