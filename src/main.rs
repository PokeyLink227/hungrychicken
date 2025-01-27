use iced::widget::{button, column, container, row, scrollable, text, Column};
use iced::{Border, Center, Element, Length, Task, Theme};
use std::time::{Duration, Instant};

pub fn main() -> iced::Result {
    iced::application("Hungry Chicken", App::update, App::view)
        .theme(theme)
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
                Task::perform(monitor_opentime(), |m| m)
            }
            Message::Stop => {
                self.state = AppState::Stopped;
                Task::none()
            }
            Message::Pause => {
                self.state = AppState::Paused;
                Task::none()
            }
        }
    }

    fn view(&self) -> Element<Message> {
        row![
            container(column![self.log.view(), self.info.view()]),
            container(self.control_pane.view()),
        ]
        .into()
    }
}

async fn monitor_opentime() -> Message {
    async_std::task::sleep(Duration::from_secs(5)).await;
    Message::Pause
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
        .width(Length::FillPortion(5))
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
                self.log.push_str("Start clicked\n");
            }
            Message::Stop => {
                self.log.push_str("Stop clicked\n");
            }
            Message::Pause => {
                self.log.push_str("Pause clicked\n");
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
        .width(Length::FillPortion(5))
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
        .width(Length::FillPortion(5))
        .style(bordered_box)
        .into()
    }
}
