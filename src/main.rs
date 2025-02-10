use crate::bot::{monitor_opentime, BotAction, Filter, Rule};
use iced::widget::{button, checkbox, column, container, row, scrollable, text, Column};
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
    NewRule,
    EnableRule(usize),
    DisableRule(usize),
    DeleteRule(usize),
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
    rules_pane: RulesPane,
    bot_handle: Option<iced::task::Handle>,
}

impl App {
    fn update(&mut self, message: Message) -> Task<Message> {
        // this is where you could loop over update calls to chain mwessages
        self.log.update(message);
        self.control_pane.update(message);
        self.rules_pane.update(message);
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
            _ => Task::none(),
        }
    }

    fn view(&self) -> Element<Message> {
        row![
            container(column![self.log.view(), self.info.view()]).width(Length::FillPortion(3)),
            container(column![self.control_pane.view(), self.rules_pane.view()])
                .width(Length::FillPortion(7)),
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
            _ => {}
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
struct RulesPane {
    rules: Vec<Rule>,
    enabled: Vec<bool>,
}

impl RulesPane {
    fn update(&mut self, message: Message) {
        match message {
            Message::NewRule => {
                self.rules.push(Rule {
                    name: "Test Rule".to_owned(),
                    filters: Vec::new(),
                    action: BotAction::Alert,
                });
                self.enabled.push(true);
            }
            Message::DeleteRule(i) => {
                self.rules.remove(i);
                self.enabled.remove(i);
            }
            Message::EnableRule(i) => self.enabled[i] = true,
            Message::DisableRule(i) => self.enabled[i] = false,
            _ => {}
        }
    }

    fn view(&self) -> Element<Message> {
        /*
            pick_list for dropdowns
            checkbox for enabled
        */
        container(scrollable(column![
            column(
                self.rules
                    .iter()
                    .enumerate()
                    .map(|(i, r)| r.view(i, self.enabled[i]))
            ),
            button("New Rule").on_press(Message::NewRule),
        ]))
        .style(bordered_box)
        .height(Length::FillPortion(9))
        .width(Length::Fill)
        .into()
    }
}

impl Rule {
    fn view(&self, index: usize, state: bool) -> Element<Message> {
        /*
            pick_list for dropdowns
            checkbox for enabled
        */
        container(row![
            text(&self.name),
            checkbox("Enable", state).on_toggle(move |b| if b {
                Message::EnableRule(index)
            } else {
                Message::DisableRule(index)
            }),
            button("Delete").on_press(Message::DeleteRule(index)),
        ])
        .style(bordered_box)
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
            _ => {}
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
