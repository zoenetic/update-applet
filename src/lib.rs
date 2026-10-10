// Copyright 2026 the cosmic-applet-updates authors
// SPDX-License-Identifier: GPL-3.0-only

pub mod backend;
mod config;
mod localize;
mod settings;

pub use settings::run as run_settings;

use std::time::{Duration, SystemTime};

use cosmic::{
    Element, Task, app,
    applet::{
        menu_button, padded_control,
        token::subscription::{TokenRequest, TokenUpdate, activation_token_subscription},
    },
    cctk::sctk::reexports::calloop,
    cosmic_theme::Spacing,
    iced::{
        self, Alignment, Background, Border, Color, Length, Shadow, Subscription,
        widget::{Column, column, container, row, stack},
        window,
    },
    surface, theme,
    widget::{divider, icon, indeterminate_linear, scrollable, text},
};

use backend::{InstallEvent, InstallJob, Kind, Source, Stage, Update, Updates};
use config::UpdatesAppletConfig;

const APP_ID: &str = "io.github.zoenetic.CosmicAppletUpdates";
/// Token request marker for opening our own settings window.
const SETTINGS_EXEC: &str = "--settings";
/// Lists longer than this are put in a scrollable area.
const MAX_VISIBLE_ROWS: usize = 8;

pub fn run() -> cosmic::iced::Result {
    localize::localize();

    cosmic::applet::run::<UpdatesApplet>(())
}

#[derive(Debug, Clone, PartialEq)]
enum Phase {
    Idle,
    Checking,
    Installing {
        stage: Stage,
        percentage: Option<u32>,
        offline: bool,
    },
}

struct UpdatesApplet {
    core: cosmic::app::Core,
    popup: Option<window::Id>,
    token_tx: Option<calloop::channel::Sender<TokenRequest>>,
    config: UpdatesAppletConfig,
    updates: Updates,
    phase: Phase,
    last_checked: Option<SystemTime>,
    /// A check was requested while busy; run it once the current work ends.
    check_pending: bool,
    install_job: Option<InstallJob>,
    next_nonce: u64,
    install_error: Option<String>,
    show_system: bool,
    show_flatpak: bool,
}

#[derive(Debug, Clone)]
enum Message {
    TogglePopup,
    CloseRequested(window::Id),
    Surface(surface::Action<Message>),
    Token(TokenUpdate),
    ConfigChanged(UpdatesAppletConfig),
    /// Check for updates; `true` refreshes the repository metadata first.
    Check(bool),
    Checked(Updates),
    Install {
        offline: bool,
    },
    Installing(InstallEvent),
    Restart,
    OpenSettings,
    OpenStore,
    ToggleSection(Source),
}

impl cosmic::Application for UpdatesApplet {
    type Executor = cosmic::SingleThreadExecutor;
    type Flags = ();
    type Message = Message;
    const APP_ID: &'static str = APP_ID;

    fn core(&self) -> &cosmic::app::Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut cosmic::app::Core {
        &mut self.core
    }

    fn init(core: cosmic::app::Core, _flags: ()) -> (Self, app::Task<Self::Message>) {
        let mut applet = Self {
            core,
            popup: None,
            token_tx: None,
            config: UpdatesAppletConfig::load(APP_ID),
            updates: Updates::default(),
            phase: Phase::Idle,
            last_checked: None,
            check_pending: false,
            install_job: None,
            next_nonce: 0,
            install_error: None,
            show_system: false,
            show_flatpak: false,
        };
        let task = applet.check(true);
        (applet, task)
    }

    fn on_close_requested(&self, id: window::Id) -> Option<Message> {
        Some(Message::CloseRequested(id))
    }

    fn update(&mut self, message: Self::Message) -> app::Task<Self::Message> {
        match message {
            Message::TogglePopup => {
                if let Some(p) = self.popup.take() {
                    return cosmic::surface::surface_task(cosmic::surface::action::destroy_popup(
                        p,
                    ));
                }
                return cosmic::surface::surface_task(cosmic::surface::action::app_popup(
                    |_| Default::default(),
                    |app: &mut UpdatesApplet| {
                        let new_id = window::Id::unique();
                        app.popup.replace(new_id);

                        app.core.applet.get_popup_settings(
                            app.core.main_window_id().unwrap(),
                            new_id,
                            None,
                            None,
                            None,
                        )
                    },
                    None,
                ));
            }
            Message::CloseRequested(id) => {
                if self.popup == Some(id) {
                    self.popup = None;
                }
            }
            Message::Surface(a) => {
                return cosmic::task::message(cosmic::Action::Surface(a));
            }
            Message::Token(u) => match u {
                TokenUpdate::Init(tx) => {
                    self.token_tx = Some(tx);
                }
                TokenUpdate::Finished => {
                    self.token_tx = None;
                }
                TokenUpdate::ActivationToken { token, exec } => {
                    let mut cmd = if exec == SETTINGS_EXEC {
                        settings::command()
                    } else {
                        std::process::Command::new(exec)
                    };
                    if let Some(token) = token {
                        cmd.env("XDG_ACTIVATION_TOKEN", &token);
                        cmd.env("DESKTOP_STARTUP_ID", &token);
                    }
                    tokio::spawn(cosmic::process::spawn(cmd));
                }
            },
            Message::ConfigChanged(config) => {
                let flatpak_toggled = config.include_flatpak != self.config.include_flatpak;
                self.config = config;
                if flatpak_toggled {
                    return self.check(false);
                }
            }
            Message::Check(refresh) => return self.check(refresh),
            Message::Checked(updates) => {
                self.updates = updates;
                self.last_checked = Some(SystemTime::now());
                self.phase = Phase::Idle;
                if std::mem::take(&mut self.check_pending) {
                    return self.check(false);
                }
            }
            Message::Install { offline } => {
                if self.phase != Phase::Idle {
                    return Task::none();
                }
                let job = InstallJob {
                    nonce: self.next_nonce,
                    system_ids: self.updates.system().iter().map(|u| u.id.clone()).collect(),
                    flatpak: !self.updates.flatpak().is_empty(),
                    offline,
                };
                if job.system_ids.is_empty() && !job.flatpak {
                    return Task::none();
                }
                self.next_nonce += 1;
                self.install_error = None;
                self.phase = Phase::Installing {
                    stage: if job.system_ids.is_empty() {
                        Stage::Flatpak
                    } else {
                        Stage::System
                    },
                    percentage: None,
                    offline,
                };
                self.install_job = Some(job);
            }
            Message::Installing(event) => match event {
                InstallEvent::Progress {
                    stage: new_stage,
                    percentage: new_percentage,
                } => {
                    if let Phase::Installing {
                        stage, percentage, ..
                    } = &mut self.phase
                    {
                        *stage = new_stage;
                        *percentage = new_percentage;
                    }
                }
                InstallEvent::Finished => {
                    self.install_job = None;
                    self.phase = Phase::Idle;
                    return self.check(false);
                }
                InstallEvent::Failed(why) => {
                    tracing::error!("failed to install updates: {why}");
                    self.install_job = None;
                    self.install_error = Some(why);
                    self.phase = Phase::Idle;
                    return self.check(false);
                }
            },
            Message::Restart => {
                // cosmic-osd asks for confirmation before restarting.
                if let Err(why) = std::process::Command::new("cosmic-osd")
                    .arg("restart")
                    .spawn()
                {
                    tracing::error!("failed to spawn cosmic-osd: {why}");
                }
            }
            Message::OpenSettings => self.launch(SETTINGS_EXEC),
            Message::OpenStore => self.launch("cosmic-store"),
            Message::ToggleSection(Source::System) => self.show_system = !self.show_system,
            Message::ToggleSection(Source::Flatpak) => self.show_flatpak = !self.show_flatpak,
        }
        Task::none()
    }

    fn view(&self) -> Element<'_, Message> {
        if self.hidden() {
            return self.core.applet.autosize_window(row![]).into();
        }

        let count = self.updates.count();
        let icon_name = if count == 0 {
            "io.github.zoenetic.CosmicAppletUpdates-symbolic"
        } else if self.updates.has_security() {
            "software-update-urgent-symbolic"
        } else {
            "software-update-available-symbolic"
        };

        let tooltip = if count == 0 {
            fl!("up-to-date")
        } else {
            fl!("updates-available", count = count)
        };

        let button: Element<'_, Message> = if !self.config.show_count || count == 0 {
            self.core
                .applet
                .icon_button(icon_name)
                .on_press_down(Message::TogglePopup)
                .into()
        } else {
            self.icon_with_count(icon_name, count)
        };

        let content: Element<'_, Message> = self
            .core
            .applet
            .applet_tooltip(
                button,
                tooltip,
                self.popup.is_some(),
                Message::Surface,
                None,
            )
            .into();

        // In the overflow popup the panel sets both bounds, and autosizing
        // would fight it.
        let in_overflow = self
            .core
            .applet
            .suggested_bounds
            .as_ref()
            .is_some_and(|c| c.width > 0. && c.height > 0.);
        if in_overflow {
            content
        } else {
            self.core.applet.autosize_window(content).into()
        }
    }

    fn view_window(&self, id: window::Id) -> Element<'_, Message> {
        if self.popup != Some(id) {
            return text("").into();
        }

        let Spacing {
            space_xxs, space_s, ..
        } = theme::active().cosmic().spacing;
        let separator =
            || padded_control(divider::horizontal::default()).padding([space_xxs, space_s]);
        let busy = self.phase != Phase::Idle;

        let mut content = column![self.status_view()]
            .align_x(Alignment::Start)
            .padding([8, 0]);

        // Update lists
        let mut has_section = false;
        if let Some(system) = &self.updates.system {
            content = content.push(separator());
            content = content.push(section(
                fl!("system-packages"),
                Source::System,
                system,
                self.show_system,
            ));
            has_section = true;
        } else {
            content = content.push(separator()).push(padded_control(
                text::caption(fl!("no-packagekit")).width(Length::Fill),
            ));
        }
        if self.config.include_flatpak
            && let Some(flatpak) = &self.updates.flatpak
        {
            if !has_section {
                content = content.push(separator());
            }
            content = content.push(section(
                fl!("flatpak-apps"),
                Source::Flatpak,
                flatpak,
                self.show_flatpak,
            ));
        }

        // Actions
        content = content.push(separator());
        if self.updates.offline_prepared && !busy {
            content = content
                .push(padded_control(
                    text::body(fl!("offline-prepared")).width(Length::Fill),
                ))
                .push(menu_button(text::body(fl!("restart-now"))).on_press(Message::Restart));
        }
        if self.updates.count() > 0 {
            content = content.push(
                menu_button(text::body(fl!("install-now")))
                    .on_press_maybe((!busy).then_some(Message::Install { offline: false })),
            );
            if !self.updates.system().is_empty() && !self.updates.offline_prepared {
                content = content.push(
                    menu_button(text::body(fl!("install-on-restart")))
                        .on_press_maybe((!busy).then_some(Message::Install { offline: true })),
                );
            }
        }
        content = content.push(
            menu_button(text::body(fl!("check-now")))
                .on_press_maybe((!busy).then_some(Message::Check(true))),
        );

        content = content
            .push(separator())
            .push(menu_button(text::body(fl!("settings"))).on_press(Message::OpenSettings))
            .push(menu_button(text::body(fl!("open-store"))).on_press(Message::OpenStore));

        self.core.applet.popup_container(content).into()
    }

    fn subscription(&self) -> Subscription<Message> {
        let mut subscriptions = vec![
            activation_token_subscription(0).map(Message::Token),
            self.core.watch_config(Self::APP_ID).map(|u| {
                for err in u.errors {
                    tracing::error!(?err, "Error watching config");
                }
                Message::ConfigChanged(u.config)
            }),
            backend::updates_changed().map(|()| Message::Check(false)),
        ];
        if self.config.check_interval_hours > 0 {
            subscriptions.push(
                backend::timer(self.config.check_interval_hours).map(|()| Message::Check(true)),
            );
        }
        if let Some(job) = &self.install_job {
            subscriptions.push(backend::install(job.clone()).map(Message::Installing));
        }
        Subscription::batch(subscriptions)
    }

    fn style(&self) -> Option<iced::theme::Style> {
        Some(cosmic::applet::style())
    }
}

impl UpdatesApplet {
    /// Starts `exec` with an activation token so its window gets focus.
    fn launch(&self, exec: &str) {
        if let Some(tx) = self.token_tx.as_ref() {
            let _ = tx.send(TokenRequest {
                app_id: APP_ID.to_string(),
                exec: exec.to_string(),
            });
        } else {
            tracing::error!("Wayland tx is None");
        }
    }

    /// Nothing to install and nothing to report, so take no panel space.
    fn hidden(&self) -> bool {
        self.config.hide_when_up_to_date
            && self.popup.is_none()
            && self.updates.count() == 0
            && !self.updates.offline_prepared
            && self.updates.errors.is_empty()
            && self.install_error.is_none()
            && !matches!(self.phase, Phase::Installing { .. })
    }

    /// Panel icon with the update count as a badge in its corner.
    ///
    /// The badge sits on top of the icon so the applet keeps the standard
    /// icon size and can never be clipped by the panel.
    fn icon_with_count<'a>(&self, icon_name: &'a str, count: usize) -> Element<'a, Message> {
        let label = if count > 99 {
            "99+".to_string()
        } else {
            count.to_string()
        };

        let badge = container(text(label).size(9).font(cosmic::font::bold()))
            .padding([0, 3])
            .class(cosmic::style::Container::Custom(Box::new(|theme| {
                let cosmic = theme.cosmic();
                container::Style {
                    text_color: Some(cosmic.accent.on.into()),
                    background: Some(Background::Color(cosmic.accent_color().into())),
                    border: Border {
                        radius: 6.0.into(),
                        width: 0.0,
                        color: Color::TRANSPARENT,
                    },
                    shadow: Shadow::default(),
                    icon_color: None,
                    snap: true,
                }
            })));

        stack![
            self.core
                .applet
                .icon_button(icon_name)
                .on_press_down(Message::TogglePopup),
            container(badge)
                .width(Length::Fill)
                .height(Length::Fill)
                .align_x(Alignment::End)
                .align_y(Alignment::Start)
                .padding(1),
        ]
        .into()
    }

    fn check(&mut self, refresh: bool) -> app::Task<Message> {
        if self.phase != Phase::Idle {
            self.check_pending = true;
            return Task::none();
        }
        self.phase = Phase::Checking;
        let include_flatpak = self.config.include_flatpak;
        Task::perform(backend::check(refresh, include_flatpak), |updates| {
            cosmic::action::app(Message::Checked(updates))
        })
    }

    /// Header of the popup: what the applet is doing or what it found.
    fn status_view(&self) -> Element<'_, Message> {
        let mut status = Column::new().spacing(4).width(Length::Fill);

        match &self.phase {
            Phase::Checking => {
                status = status
                    .push(text::heading(fl!("checking")))
                    .push(indeterminate_linear().width(Length::Fill));
            }
            Phase::Installing {
                stage,
                percentage,
                offline,
            } => {
                let label = match (stage, offline) {
                    (Stage::System, true) => fl!("downloading-system"),
                    (Stage::System, false) => fl!("installing-system"),
                    (Stage::Flatpak, _) => fl!("installing-flatpak"),
                };
                let progress = match percentage {
                    Some(p) => indeterminate_linear().progress(*p as f32 / 100.0),
                    None => indeterminate_linear(),
                };
                status = status
                    .push(text::heading(label))
                    .push(progress.width(Length::Fill));
            }
            Phase::Idle => {
                let count = self.updates.count();
                if count == 0 {
                    status = status.push(text::heading(fl!("up-to-date")));
                } else {
                    status = status.push(text::heading(fl!("updates-available", count = count)));
                    let security = self
                        .updates
                        .system()
                        .iter()
                        .filter(|u| u.kind == Kind::Security)
                        .count();
                    if security > 0 {
                        status =
                            status.push(text::caption(fl!("security-updates", count = security)));
                    }
                }
                if let Some(checked) = self.last_checked {
                    status = status.push(text::caption(last_checked(checked)));
                }
            }
        }

        let errors = self.install_error.iter().chain(self.updates.errors.iter());
        for why in errors {
            status = status.push(text::caption(fl!("error", details = why.as_str())));
        }

        padded_control(status).into()
    }
}

/// A collapsible list of updates from one source.
fn section<'a>(
    title: String,
    source: Source,
    updates: &'a [Update],
    expanded: bool,
) -> Element<'a, Message> {
    let dropdown_icon = if expanded {
        "go-up-symbolic"
    } else {
        "go-down-symbolic"
    };
    let header = menu_button(row![
        text::body(format!("{title} ({})", updates.len()))
            .width(Length::Fill)
            .height(Length::Fixed(24.0))
            .align_y(Alignment::Center),
        container(icon::from_name(dropdown_icon).size(16).symbolic(true))
            .center(Length::Fixed(24.0))
    ])
    .on_press_maybe((!updates.is_empty()).then_some(Message::ToggleSection(source)));

    if !expanded || updates.is_empty() {
        return header.into();
    }

    let rows = Column::with_children(updates.iter().map(update_row));
    let list: Element<'_, Message> = if updates.len() > MAX_VISIBLE_ROWS {
        scrollable(rows).height(Length::Fixed(300.0)).into()
    } else {
        rows.into()
    };

    column![header, list].into()
}

fn update_row(update: &Update) -> Element<'_, Message> {
    let mut line = row![
        column![
            text::body(&update.name),
            text::caption(format!("{} · {}", update.version, update.summary)),
        ]
        .width(Length::Fill)
    ]
    .align_y(Alignment::Center)
    .spacing(8);

    if update.kind == Kind::Security {
        line = line.push(
            icon::from_name("security-high-symbolic")
                .size(16)
                .symbolic(true),
        );
    }

    padded_control(line).into()
}

fn last_checked(at: SystemTime) -> String {
    let minutes = at.elapsed().unwrap_or(Duration::ZERO).as_secs() / 60;
    if minutes < 60 {
        fl!("last-checked", minutes = minutes)
    } else {
        let hours = minutes / 60;
        fl!("last-checked-hours", hours = hours)
    }
}
