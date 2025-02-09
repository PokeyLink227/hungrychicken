use crate::bot::monitor_opentime;
use iced::widget::{button, column, container, row, scrollable, text, Column};
use iced::{
    keyboard::{key, on_key_press, Key, Modifiers},
    Border, Center, Element, Length, Subscription, Task, Theme,
};
use std::time::{Duration, Instant};

mod bot;

pub fn main() -> iced::Result {
    iced::application("Hungry Chicken", App::update, App::view)
        .theme(theme)
        .subscription(App::subscription)
        .run()
}

fn theme(_state: &App) -> Theme {
    iced::Theme::TokyoNightStorm
}

fn bordered_box(theme: &Theme) -> container::Style {
    let s = container::bordered_box(theme);
    s
}

#[derive(Debug, Clone, Copy)]
enum MonitorMessage {
    Start,
    Stop,
    Pause,
}

#[derive(Debug, Clone, Copy)]
enum Message {
    Start,
    Stop,
    Pause,
}

#[derive(Debug, Default, Eq, PartialEq)]
enum AppState {
    #[default]
    Stopped,
    Running,
    Paused,
}

#[derive(Debug, Default)]
struct App {
    state: AppState,
    log: LogPane,
    info: InfoPane,
    control_pane: ControlPane,
    bot_handle: Option<iced::task::Handle>,
}

impl App {
    fn update(&mut self, message: Message) -> Task<Message> {
        // this is where you could loop over update calls to chain mwessages
        self.log.update(message);
        self.control_pane.update(message);
        //self.info.update();

        match message {
            Message::Start => {
                self.state = AppState::Running;
                if self.bot_handle.is_none() {
                    let (t, h) = Task::abortable(Task::perform(monitor_opentime(), |m| m));
                    self.bot_handle = Some(h);
                    t
                } else {
                    Task::none()
                }
            }
            Message::Stop => {
                self.state = AppState::Stopped;
                if let Some(h) = &self.bot_handle {
                    h.abort();
                    self.bot_handle = None;
                }
                Task::none()
            }
            Message::Pause => {
                self.state = AppState::Paused;
                if let Some(h) = &self.bot_handle {
                    h.abort();
                    self.bot_handle = None;
                }
                Task::none()
            }
        }
    }

    fn view(&self) -> Element<Message> {
        row![
            container(column![self.log.view(), self.info.view()]).width(Length::FillPortion(3)),
            container(self.control_pane.view()).width(Length::FillPortion(7)),
        ]
        .into()
    }

    fn subscription(&self) -> Subscription<Message> {
        on_key_press(|key, mods| match key {
            Key::Named(key::Named::Escape) => Some(Message::Stop),
            _ => None,
        })
    }
}

#[derive(Default, Debug)]
struct ControlPane {
    state: AppState,
}

impl ControlPane {
    fn update(&mut self, message: Message) {
        match message {
            Message::Start => {
                self.state = AppState::Running;
            }
            Message::Stop => {
                self.state = AppState::Stopped;
            }
            Message::Pause => {
                self.state = AppState::Paused;
            }
        }
    }

    fn view(&self) -> Element<Message> {
        container(row![
            text(format!("Current State: {:?}", self.state)).size(20),
            if self.state == AppState::Running {
                button("Start")
            } else {
                button("Start").on_press(Message::Start)
            },
            if self.state == AppState::Stopped {
                button("Stop")
            } else {
                button("Stop").on_press(Message::Stop)
            },
            if self.state == AppState::Paused {
                button("Pause")
            } else {
                button("Pause").on_press(Message::Pause)
            },
        ])
        .style(bordered_box)
        .height(Length::FillPortion(1))
        .width(Length::Fill)
        .into()
    }
}

#[derive(Default, Debug)]
struct LogPane {
    log: String,
}

impl LogPane {
    fn update(&mut self, message: Message) {
        match message {
            Message::Start => {
                self.log.push_str("Starting Bot\n");
            }
            Message::Stop => {
                self.log.push_str("Bot Stopped\n");
            }
            Message::Pause => {
                self.log.push_str("Paused\n");
            }
        }
    }

    fn view(&self) -> Element<Message> {
        container(
            scrollable(text(&self.log))
                .anchor_bottom()
                .width(Length::Fill),
        )
        .height(Length::FillPortion(7))
        .width(Length::Fill)
        .style(bordered_box)
        .into()
    }
}

#[derive(Debug)]
struct InfoPane {
    start_time: Instant,
    num_refreshes: u32,
}

impl Default for InfoPane {
    fn default() -> InfoPane {
        InfoPane {
            start_time: Instant::now(),
            num_refreshes: 0,
        }
    }
}

impl InfoPane {
    fn update(&mut self, message: Message) {}

    fn view(&self) -> Element<Message> {
        container(column![
            text(format!("Uptime: {:?}", self.start_time.elapsed().as_secs())).size(15),
            text(format!("Refreshes: {:?}", self.num_refreshes)).size(15),
        ])
        .height(Length::FillPortion(2))
        .width(Length::Fill)
        .style(bordered_box)
        .into()
    }
}
