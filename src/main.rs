use iced::widget::{button, column, row, text, Column, scrollable};
use iced::Center;

pub fn main() -> iced::Result {
    iced::application("Hungry Chicken", App::update, App::view)
        .run()
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
            text(format!("Current State: {:?}", self.state)).size(50),
            row![
                button("Start").on_press(Message::Start),
                button("Stop").on_press(Message::Stop),
                button("Pause").on_press(Message::Pause),
            ]
            .padding(65),
            scrollable(text(&self.log)).anchor_bottom(),
        ]
        .padding(20)
        .align_x(Center)
    }
}



#[derive(Debug, Default)]
struct AppInfo {
    start_time: u32,
    num_refreshes: u32,
}
