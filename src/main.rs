use iced::widget::{button, column, row, scrollable, text, Column};
use iced::{Center, Element, Length, Theme};

pub fn main() -> iced::Result {
    iced::application("Hungry Chicken", App::update, App::view)
        .theme(theme)
        .run()
}

fn theme(_state: &App) -> Theme {
    iced::Theme::TokyoNightStorm
}

#[derive(Debug, Clone, Copy)]
enum Message {
    Start,
    Stop,
    Pause,
}

#[derive(Debug, Default)]
enum AppState {
    #[default]
    Stopped,
    Running,
    Paused,
}

#[derive(Debug, Default)]
struct App {
    state: AppState,
    log: String,
    info: AppInfo,
}

impl App {
    fn update(&mut self, message: Message) {
        match message {
            Message::Start => {
                self.state = AppState::Running;
                self.log.push_str("Start clicked\n");
            }
            Message::Stop => {
                self.state = AppState::Stopped;
                self.log.push_str("Stop clicked\n");
            }
            Message::Pause => {
                self.state = AppState::Paused;
                self.log.push_str("Pause clicked\n");
            }
        }
    }

    fn view(&self) -> Column<Message> {
        column![
            row![
                text(format!("Current State: {:?}", self.state)).size(20),
                button("Start").on_press(Message::Start),
                button("Stop").on_press(Message::Stop),
                button("Pause").on_press(Message::Pause),
            ]
            .height(Length::FillPortion(1)),
            scrollable(text(&self.log))
                .anchor_bottom()
                .height(Length::FillPortion(7)),
            self.info.view(),
        ]
        .width(Length::FillPortion(5))
    }
}

#[derive(Debug, Default)]
struct AppInfo {
    start_time: u32,
    num_refreshes: u32,
}

impl AppInfo {
    fn update(&mut self, message: Message) {}

    fn view(&self) -> Element<Message> {
        column![
            text(format!("Uptime: {:?}", self.start_time)).size(15),
            text(format!("Refreshes: {:?}", self.num_refreshes)).size(15),
        ]
        .height(Length::FillPortion(2))
        .into()
    }
}
