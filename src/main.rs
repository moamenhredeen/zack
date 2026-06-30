mod home_screen;
mod routes;
mod settings_screen;

use gpui::*;
use gpui_component::{
    sidebar::{Sidebar, SidebarFooter, SidebarGroup, SidebarHeader, SidebarMenu, SidebarMenuItem},
    *,
};

use crate::routes::Routes;
use crate::settings_screen::SettingsScreen;
use crate::{home_screen::HomeScreen, routes::Navigate};

pub struct AppShell {
    active_route: Routes,
    home_screen: Entity<HomeScreen>,
    settings_screen: Entity<SettingsScreen>,
}

impl AppShell {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let home_screen = cx.new(|cx| HomeScreen::new(window, cx));
        let settings_screen = cx.new(|_| SettingsScreen);

        cx.subscribe(&home_screen, Self::on_navigate).detach();
        cx.subscribe(&settings_screen, Self::on_navigate).detach();

        Self {
            active_route: Routes::Home,
            home_screen,
            settings_screen,
        }
    }

    pub fn on_navigate(
        app_shell: &mut AppShell,
        _: Entity<impl EventEmitter<Navigate>>,
        event: &Navigate,
        cx: &mut Context<AppShell>,
    ) {
        app_shell.navigate(cx, event.0);
    }

    pub fn navigate(&mut self, cx: &mut Context<Self>, route: Routes) {
        self.active_route = route;
        cx.notify();
    }

    pub fn active_screen(&mut self) -> AnyView {
        match self.active_route {
            Routes::Home => self.home_screen.clone().into(),
            Routes::Settings => self.settings_screen.clone().into(),
        }
    }
}

impl Render for AppShell {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .size_full()
            .child(TitleBar::new().child("Zack"))
            .child(
                div()
                    .flex()
                    .flex_1()
                    .gap_2()
                    .child(
                        Sidebar::new("sidebar-id")
                            .w(px(250.))
                            .h_full()
                            .header(SidebarHeader::new().child("Zack App"))
                            .footer(SidebarFooter::new().child("Zack"))
                            .child(
                                SidebarGroup::new("nav-items").child(
                                    SidebarMenu::new()
                                        .child(
                                            SidebarMenuItem::new("Home")
                                                .icon(IconName::LayoutDashboard)
                                                .active(self.active_route == Routes::Home)
                                                .on_click(cx.listener(|app_shell, _, _, cx| {
                                                    app_shell.navigate(cx, Routes::Home);
                                                })),
                                        )
                                        .child(
                                            SidebarMenuItem::new("Settings")
                                                .icon(IconName::Folder)
                                                .active(self.active_route == Routes::Settings)
                                                .on_click(cx.listener(|app_shell, _, _, cx| {
                                                    app_shell.navigate(cx, Routes::Settings);
                                                })),
                                        ),
                                ),
                            ),
                    )
                    .child(self.active_screen()),
            )
    }
}

fn main() {
    let app = gpui_platform::application().with_assets(gpui_component_assets::Assets);

    app.run(move |cx| {
        gpui_component::init(cx);

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
