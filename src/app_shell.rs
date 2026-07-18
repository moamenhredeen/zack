use crate::collection::CollectionStore;
use crate::fonts::JETBRAINS_MONO;
use crate::settings::Settings;
use crate::workspace::WorkspaceScreen;
use gpui::{AppContext, Context, Entity, IntoElement, ParentElement, Render, Styled, Window};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::{ActiveTheme, IconName, Root, Selectable, TitleBar, h_flex, v_flex};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Page {
    Collection,
    Environment,
    Settings,
}

pub struct AppShell {
    active_page: Page,
    collections: Entity<CollectionStore>,
    workspace: Entity<WorkspaceScreen>,
    settings: Entity<Settings>,
}

impl AppShell {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let collections = cx.new(|_| CollectionStore::new());
        Self {
            active_page: Page::Collection,
            workspace: cx.new(|cx| WorkspaceScreen::new(collections.clone(), window, cx)),
            collections,
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
