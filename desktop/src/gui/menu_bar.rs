use crate::custom_event::RuffleEvent;
use crate::gui::dialogs::Dialogs;
use crate::gui::{text, DebugMessage};
use crate::player::LaunchOptions;
use crate::preferences::GlobalPreferences;
use egui::{menu, Button, Key, KeyboardShortcut, Modifiers, Widget};
use ruffle_core::Player;
use ruffle_frontend_utils::recents::Recent;
use unic_langid::LanguageIdentifier;
use url::Url;
use winit::event_loop::EventLoopProxy;

pub struct MenuBar {
    event_loop: EventLoopProxy<RuffleEvent>,
    default_launch_options: LaunchOptions,
    preferences: GlobalPreferences,

    cached_recents: Option<Vec<Recent>>,
    pub currently_opened: Option<(Url, LaunchOptions)>,
}

impl MenuBar {
    pub fn new(
        event_loop: EventLoopProxy<RuffleEvent>,
        default_launch_options: LaunchOptions,
        preferences: GlobalPreferences,
    ) -> Self {
        Self {
            event_loop,
            default_launch_options,
            cached_recents: None,
            currently_opened: None,
            preferences,
        }
    }

    pub fn show(
        &mut self,
        locale: &LanguageIdentifier,
        egui_ctx: &egui::Context,
        dialogs: &mut Dialogs,
        mut player: Option<&mut Player>,
    ) {
        egui::TopBottomPanel::top("menu_bar").show(egui_ctx, |ui| {
            // TODO(mike): Make some MenuItem struct with shortcut info to handle this more cleanly.
            if ui.ctx().input_mut(|input| {
                input.consume_shortcut(&KeyboardShortcut::new(Modifiers::COMMAND | Modifiers::SHIFT, Key::O))
            }) {
                dialogs.open_file_advanced();
            }
            if ui.ctx().input_mut(|input| {
                input.consume_shortcut(&KeyboardShortcut::new(Modifiers::COMMAND, Key::O))
            }) {
                self.open_file(ui);
            }
            if ui.ctx().input_mut(|input| {
                input.consume_shortcut(&KeyboardShortcut::new(Modifiers::COMMAND, Key::Q))
            }) {
                self.request_exit(ui);
            }
            if ui.ctx().input_mut(|input| {
                input.consume_shortcut(&KeyboardShortcut::new(Modifiers::COMMAND, Key::P))
            }) {
                if let Some(player) = &mut player {
                    player.set_is_playing(!player.is_playing());
                }
            }

            menu::bar(ui, |ui| {
                self.file_menu(locale, ui, dialogs, player.is_some());

                menu::menu_button(ui, text(locale, "controls-menu"), |ui| {
                    ui.add_enabled_ui(player.is_some(), |ui| {
                        let playing = player.as_ref().map(|p| p.is_playing()).unwrap_or_default();
                        let pause_shortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::P);
                        if Button::new(text(locale, if playing { "controls-menu-suspend" } else { "controls-menu-resume" })).shortcut_text(ui.ctx().format_shortcut(&pause_shortcut)).ui(ui).clicked() {
                            ui.close_menu();
                            if let Some(player) = &mut player {
                                player.set_is_playing(!player.is_playing());
                            }
                        }
                    });
                    if Button::new(text(locale, "controls-menu-volume")).ui(ui).clicked() {
                        dialogs.open_volume_controls();
                        ui.close_menu();
                    }
                });
                menu::menu_button(ui, text(locale, "bookmarks-menu"), |ui| {
                    if Button::new(text(locale, "bookmarks-menu-add")).ui(ui).clicked() {
                        ui.close_menu();

                        let initial_url = self.currently_opened.as_ref().map(|(url, _)| url.clone());

                        dialogs.open_add_bookmark(initial_url);
                    }

                    if Button::new(text(locale, "bookmarks-menu-manage")).ui(ui).clicked() {
                        ui.close_menu();
                        dialogs.open_bookmarks();
                    }

                    if self.preferences.have_bookmarks() {
                        ui.separator();
                        self.preferences.bookmarks(|bookmarks| {
                            for bookmark in bookmarks.iter().filter(|x| !x.is_invalid()) {
                                if Button::new(&bookmark.name).ui(ui).clicked() {
                                    ui.close_menu();
                                    let _ = self.event_loop.send_event(RuffleEvent::OpenURL(bookmark.url.clone(), Box::new(self.default_launch_options.clone())));
                                }
                            }
                        });
                    }
                });
                menu::menu_button(ui, text(locale, "debug-menu"), |ui| {
                    ui.add_enabled_ui(player.is_some(), |ui| {
                        if Button::new(text(locale, "debug-menu-open-stage")).ui(ui).clicked() {
                            ui.close_menu();
                            if let Some(player) = &mut player {
                                player.debug_ui().queue_message(DebugMessage::TrackStage);
                            }
                        }
                        if Button::new(text(locale, "debug-menu-open-movie")).ui(ui).clicked() {
                            ui.close_menu();
                            if let Some(player) = &mut player {
                                player.debug_ui().queue_message(DebugMessage::TrackTopLevelMovie);
                            }
                        }
                        if Button::new(text(locale, "debug-menu-open-movie-list")).ui(ui).clicked() {
                            ui.close_menu();
                            if let Some(player) = &mut player {
                                player.debug_ui().queue_message(DebugMessage::ShowKnownMovies);
                            }
                        }
                        if Button::new(text(locale, "debug-menu-open-domain-list")).ui(ui).clicked() {
                            ui.close_menu();
                            if let Some(player) = &mut player {
                                player.debug_ui().queue_message(DebugMessage::ShowDomains);
                            }
                        }
                        if Button::new("Show Sockets").ui(ui).clicked() {
                            ui.close_menu();
                            if let Some(player) = &mut player {
                                player.debug_ui().queue_message(DebugMessage::ShowSockets);
                            }
                        }
                        if Button::new(text(locale, "debug-menu-search-display-objects")).ui(ui).clicked() {
                            ui.close_menu();
                            if let Some(player) = &mut player {
                                player.debug_ui().queue_message(DebugMessage::SearchForDisplayObject);
                            }
                        }
                    });
                });
                menu::menu_button(ui, text(locale, "help-menu"), |ui| {
                    if ui.button(text(locale, "help-menu-join-discord")).clicked() {
                        self.launch_website(ui, "https://discord.gg/ruffle");
                    }
                    if ui.button(text(locale, "help-menu-report-a-bug")).clicked() {
                        self.launch_website(ui, "https://github.com/ruffle-rs/ruffle/issues/new?assignees=&labels=bug&projects=&template=bug_report.yml");
                    }
                    if ui.button(text(locale, "help-menu-sponsor-development")).clicked() {
                        self.launch_website(ui, "https://opencollective.com/ruffle/");
                    }
                    if ui.button(text(locale, "help-menu-translate-ruffle")).clicked() {
                        self.launch_website(ui, "https://crowdin.com/project/ruffle");
                    }
                    ui.separator();
                    if ui.button(text(locale, "help-menu-about")).clicked() {
                        dialogs.open_about_screen();
                        ui.close_menu();
                    }
                });
            });
        });
    }

    fn file_menu(
        &mut self,
        locale: &LanguageIdentifier,
        ui: &mut egui::Ui,
        dialogs: &mut Dialogs,
        player_exists: bool,
    ) {
        menu::menu_button(ui, text(locale, "file-menu"), |ui| {
            let mut shortcut;

            shortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::O);
            if Button::new(text(locale, "file-menu-open-quick"))
                .shortcut_text(ui.ctx().format_shortcut(&shortcut))
                .ui(ui)
                .clicked()
            {
                self.open_file(ui);
            }

            shortcut = KeyboardShortcut::new(Modifiers::COMMAND | Modifiers::SHIFT, Key::O);
            if Button::new(text(locale, "file-menu-open-advanced"))
                .shortcut_text(ui.ctx().format_shortcut(&shortcut))
                .ui(ui)
                .clicked()
            {
                ui.close_menu();
                dialogs.open_file_advanced();
            }

            if ui
                .add_enabled(player_exists, Button::new(text(locale, "file-menu-reload")))
                .clicked()
            {
                self.reload_movie(ui);
            }

            if ui
                .add_enabled(player_exists, Button::new(text(locale, "file-menu-close")))
                .clicked()
            {
                self.close_movie(ui);
            }
            ui.separator();

            let recent_menu_response = ui
                .menu_button(text(locale, "file-menu-recents"), |ui| {
                    if self
                        .cached_recents
                        .as_ref()
                        .map(|x| x.is_empty())
                        .unwrap_or(true)
                    {
                        ui.label(text(locale, "file-menu-recents-empty"));
                    }

                    if let Some(recents) = &self.cached_recents {
                        for recent in recents {
                            if ui.button(&recent.name).clicked() {
                                ui.close_menu();
                                let _ = self.event_loop.send_event(RuffleEvent::OpenURL(
                                    recent.url.clone(),
                                    Box::new(self.default_launch_options.clone()),
                                ));
                            }
                        }
                    };
                })
                .inner;

            match recent_menu_response {
                // recreate the cache on the first draw.
                Some(_) if self.cached_recents.is_none() => {
                    self.cached_recents = Some(self.preferences.recents(|recents| {
                        recents
                            .iter()
                            .rev()
                            .filter(|x| !x.is_invalid() && x.is_available())
                            .cloned()
                            .collect::<Vec<_>>()
                    }))
                }
                // clear cache, since menu was closed.
                None if self.cached_recents.is_some() => self.cached_recents = None,
                _ => {}
            }

            ui.separator();
            if Button::new(text(locale, "file-menu-preferences"))
                .ui(ui)
                .clicked()
            {
                ui.close_menu();
                dialogs.open_preferences();
            }
            ui.separator();

            shortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::Q);
            if Button::new(text(locale, "file-menu-exit"))
                .shortcut_text(ui.ctx().format_shortcut(&shortcut))
                .ui(ui)
                .clicked()
            {
                self.request_exit(ui);
            }
        });
    }

    fn open_file(&mut self, ui: &mut egui::Ui) {
        ui.close_menu();

        let _ = self
            .event_loop
            .send_event(RuffleEvent::BrowseAndOpen(Box::new(
                self.default_launch_options.clone(),
            )));
    }

    fn close_movie(&mut self, ui: &mut egui::Ui) {
        let _ = self.event_loop.send_event(RuffleEvent::CloseFile);
        self.currently_opened = None;
        ui.close_menu();
    }

    fn reload_movie(&mut self, ui: &mut egui::Ui) {
        let _ = self.event_loop.send_event(RuffleEvent::CloseFile);
        if let Some((movie_url, opts)) = self.currently_opened.take() {
            let _ = self
                .event_loop
                .send_event(RuffleEvent::OpenURL(movie_url, opts.into()));
        }
        ui.close_menu();
    }

    fn request_exit(&mut self, ui: &mut egui::Ui) {
        let _ = self.event_loop.send_event(RuffleEvent::ExitRequested);
        ui.close_menu();
    }

    fn launch_website(&mut self, ui: &mut egui::Ui, url: &str) {
        let _ = webbrowser::open(url);
        ui.close_menu();
    }
}
