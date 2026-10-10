// Copyright 2026 the cosmic-applet-updates authors
// SPDX-License-Identifier: GPL-3.0-only

//! Standalone settings window, opened with `cosmic-applet-updates --settings`.
//!
//! It edits the same cosmic-config entry as the applet, so changes apply to
//! the applet immediately, and it works while the applet is hidden.

use cosmic::{
    Element, Task, app,
    iced::{Length, Size, Subscription},
    widget::{self, settings},
};

use cosmic::app::ApplicationExt;

use crate::{
    APP_ID,
    config::{self, CHECK_INTERVALS, UpdatesAppletConfig},
    fl,
};

const SETTINGS_APP_ID: &str = "io.github.zoenetic.CosmicAppletUpdates.Settings";

pub fn run() -> cosmic::iced::Result {
    crate::localize::localize();

    let settings = cosmic::app::Settings::default().size(Size::new(520.0, 440.0));
    cosmic::app::run::<SettingsApp>(settings, ())
}

/// Command that opens this window, used by the applet's "Settings…" item.
pub fn command() -> std::process::Command {
    let exe = std::env::current_exe().unwrap_or_else(|_| "cosmic-applet-updates".into());
    let mut cmd = std::process::Command::new(exe);
    cmd.arg("--settings");
    cmd
}

struct SettingsApp {
    core: cosmic::app::Core,
    config: UpdatesAppletConfig,
    /// Labels for [`CHECK_INTERVALS`], in the same order.
    intervals: Vec<String>,
}

#[derive(Debug, Clone)]
enum Message {
    ConfigChanged(UpdatesAppletConfig),
    HideWhenUpToDate(bool),
    ShowCount(bool),
    IncludeFlatpak(bool),
    CheckInterval(usize),
}

impl cosmic::Application for SettingsApp {
    type Executor = cosmic::executor::Default;
    type Flags = ();
    type Message = Message;
    const APP_ID: &'static str = SETTINGS_APP_ID;

    fn core(&self) -> &cosmic::app::Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut cosmic::app::Core {
        &mut self.core
    }

    fn init(core: cosmic::app::Core, _flags: ()) -> (Self, app::Task<Self::Message>) {
        let mut app = Self {
            core,
            config: UpdatesAppletConfig::load(APP_ID),
            intervals: CHECK_INTERVALS
                .iter()
                .map(|&hours| fl!("check-every", hours = hours))
                .collect(),
        };

        let title = fl!("settings-title");
        app.set_header_title(title.clone());
        let task = match app.core.main_window_id() {
            Some(id) => app.set_window_title(title, id),
            None => Task::none(),
        };
        (app, task)
    }

    fn update(&mut self, message: Self::Message) -> app::Task<Self::Message> {
        let config = &mut self.config;
        match message {
            Message::ConfigChanged(new) => *config = new,
            Message::HideWhenUpToDate(value) => {
                config::save(config, APP_ID, |c, h| c.set_hide_when_up_to_date(h, value));
            }
            Message::ShowCount(value) => {
                config::save(config, APP_ID, |c, h| c.set_show_count(h, value));
            }
            Message::IncludeFlatpak(value) => {
                config::save(config, APP_ID, |c, h| c.set_include_flatpak(h, value));
            }
            Message::CheckInterval(index) => {
                if let Some(&hours) = CHECK_INTERVALS.get(index) {
                    config::save(config, APP_ID, |c, h| c.set_check_interval_hours(h, hours));
                }
            }
        }
        Task::none()
    }

    fn view(&self) -> Element<'_, Message> {
        let selected_interval = CHECK_INTERVALS
            .iter()
            .position(|&h| h == self.config.check_interval_hours);

        let panel = settings::section()
            .title(fl!("section-panel"))
            .add(
                settings::item::builder(fl!("hide-when-up-to-date"))
                    .description(fl!("hide-when-up-to-date-description"))
                    .toggler(self.config.hide_when_up_to_date, Message::HideWhenUpToDate),
            )
            .add(
                settings::item::builder(fl!("show-count"))
                    .toggler(self.config.show_count, Message::ShowCount),
            );

        let checking = settings::section()
            .title(fl!("section-checking"))
            .add(
                settings::item::builder(fl!("check-interval")).control(widget::dropdown(
                    self.intervals.as_slice(),
                    selected_interval,
                    Message::CheckInterval,
                )),
            )
            .add(
                settings::item::builder(fl!("include-flatpak"))
                    .toggler(self.config.include_flatpak, Message::IncludeFlatpak),
            );

        widget::container(settings::view_column(vec![panel.into(), checking.into()]))
            .width(Length::Fill)
            .max_width(640)
            .padding(16)
            .into()
    }

    fn subscription(&self) -> Subscription<Message> {
        self.core
            .watch_config::<UpdatesAppletConfig>(APP_ID)
            .map(|update| {
                for err in update.errors {
                    tracing::error!(?err, "Error watching config");
                }
                Message::ConfigChanged(update.config)
            })
    }
}
