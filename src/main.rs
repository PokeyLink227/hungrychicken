use crate::bot::bot_thread;
use crate::bot::{BotAction, Date, Field, Filter, FilterType, Op, Rule, Time};
use iced::widget::{button, checkbox, column, container, row, scrollable, text, Column};
use iced::{
    keyboard::{key, on_key_press, Key, Modifiers},
    Border, Center, Color, Element, Length, Padding, Size, Subscription, Task, Theme,
};
use self_update::cargo_crate_version;
use std::sync::mpsc;
use std::sync::mpsc::Receiver;
use std::sync::mpsc::Sender;
use std::thread;
use std::time::{Duration, Instant};

mod bot;
mod update;

pub fn main() -> iced::Result {
    // handle updates
    let res = update::update();
    println!("{:?}", res);

    iced::application(title, App::update, App::view)
        .theme(theme)
        .window_size((650.0, 800.0))
        .settings(iced::settings::Settings {
            id: Some("main_window".to_string()),
            ..iced::settings::Settings::default()
        })
        .subscription(App::subscription)
        .run_with(App::init)
}

fn theme(_state: &App) -> Theme {
    iced::Theme::TokyoNightStorm
}

fn title(_state: &App) -> String {
    format!("Hungry Chicken {}", cargo_crate_version!())
}

fn bordered_box(theme: &Theme) -> container::Style {
    let mut s = container::bordered_box(theme);
    s.border = s.border.rounded(5);
    s
}

fn filter_box(theme: &Theme) -> container::Style {
    let mut s = container::bordered_box(theme);
    s.border = s.border.rounded(5);
    s.border.color = Color {
        r: 1.0,
        g: 0.0,
        b: 0.0,
        a: 1.0,
    };
    // change color
    s
}

#[derive(Debug, Clone, Copy)]
enum MonitorMessage {
    Start,
    Stop,
    Pause,
}

#[derive(Debug, Clone)]
enum Message {
    Tick,
    Start,
    Stop,
    Pause,
    NewRule,
    EnableRule(usize),
    DisableRule(usize),
    DeleteRule(usize),
    ChangeRuleAction(usize, BotAction),
    GotWindowId(iced::window::Id),
    NewFilter(usize, FilterType),
    DeleteFilter(usize, usize),
    UpdateFilter(usize, usize, Filter),
    UpdateEntry(usize, usize, String),
    SubmitEntry(usize, usize, Filter),
}

#[derive(Debug, Default, Eq, PartialEq)]
enum AppState {
    #[default]
    Stopped,
    Running,
}

#[derive(Debug)]
struct App {
    window_id: Option<iced::window::Id>,
    state: AppState,
    log: LogPane,
    info: InfoPane,
    control_pane: ControlPane,
    rules_pane: RulesPane,
    bot_handle: Option<iced::task::Handle>,
    rx: Receiver<Message>,
    tx: Sender<Message>,
}

impl App {
    fn init() -> (App, Task<Message>) {
        let (mb_tx, mb_rx) = mpsc::channel();
        let (bm_tx, bm_rx) = mpsc::channel();
        let thread_handle = thread::spawn(move || bot_thread(mb_rx, bm_tx));

        (
            App {
                window_id: None,
                state: AppState::default(),
                log: LogPane::default(),
                info: InfoPane::default(),
                control_pane: ControlPane::default(),
                rules_pane: RulesPane::default(),
                bot_handle: None,
                rx: bm_rx,
                tx: mb_tx,
            },
            Task::map(iced::window::get_latest(), |m| {
                Message::GotWindowId(m.unwrap())
            }),
        )
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        // this is where you could loop over update calls to chain mwessages
        self.log.update(message.clone());
        self.control_pane.update(message.clone());
        self.rules_pane.update(message.clone());
        //self.info.update();

        match message {
            Message::Tick => {
                if let Some(m) = self.rx.try_recv().ok() {
                    Task::done(m)
                } else {
                    Task::none()
                }
            }
            Message::Start => {
                self.state = AppState::Running;
                self.tx.send(Message::Start).unwrap();
                Task::none()
            }
            Message::Stop => {
                self.state = AppState::Stopped;
                self.tx.send(Message::Stop).unwrap();
                iced::window::gain_focus(self.window_id.unwrap())
            }
            Message::GotWindowId(i) => {
                self.window_id = Some(i);
                Task::none()
            }
            _ => Task::none(),
        }
    }

    fn view(&self) -> Element<Message> {
        row![
            container(column![self.log.view(), self.info.view()].spacing(5))
                .width(Length::FillPortion(3)),
            container(column![self.control_pane.view(), self.rules_pane.view()].spacing(5))
                .width(Length::FillPortion(7)),
        ]
        .spacing(5)
        .into()
    }

    fn subscription(&self) -> Subscription<Message> {
        iced::time::every(Duration::from_millis(10)).map(|_| Message::Tick)
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
    entries: Vec<Vec<String>>,
}

impl RulesPane {
    fn update(&mut self, message: Message) {
        match message {
            Message::NewRule => {
                self.rules.push(Rule {
                    name: "Test Rule".to_owned(),
                    filters: vec![],
                    action: BotAction::Alert,
                });
                self.enabled.push(true);
                self.entries.push(Vec::new());
            }
            Message::DeleteRule(i) => {
                self.rules.remove(i);
                self.enabled.remove(i);
                self.entries.remove(i);
            }
            Message::ChangeRuleAction(i, a) => {
                self.rules[i].action = a;
            }
            Message::EnableRule(i) => self.enabled[i] = true,
            Message::DisableRule(i) => self.enabled[i] = false,
            Message::NewFilter(i, f) => {
                self.rules[i].filters.push(f.into());
                self.entries[i].push(String::new());
            }
            Message::DeleteFilter(ri, i) => {
                self.rules[ri].filters.remove(i);
                self.entries[ri].remove(i);
            }
            Message::UpdateFilter(ri, i, f) => {
                self.rules[ri].filters[i] = f;
            }
            Message::UpdateEntry(ri, i, s) => {
                self.entries[ri][i] = s;
            }
            Message::SubmitEntry(ri, i, f) => match f {
                Filter::FieldIs(f, o, _) => {
                    if let Ok(t) = self.entries[ri][i].parse() {
                        self.rules[ri].filters[i] = Filter::FieldIs(f, o, t);
                    }
                }
                Filter::TimeDiff(f1, f2, o, _) => {
                    if let Ok(t) = self.entries[ri][i].parse() {
                        self.rules[ri].filters[i] = Filter::TimeDiff(f1, f2, o, t)
                    }
                }

                Filter::DateIs(op, _) => {
                    if let Ok(d) = self.entries[ri][i].parse() {
                        self.rules[ri].filters[i] = Filter::DateIs(op, d)
                    }
                }
                Filter::NumDays(op, _) => {
                    if let Ok(num) = self.entries[ri][i].parse() {
                        self.rules[ri].filters[i] = Filter::NumDays(op, num)
                    }
                }
                Filter::IncludeLayover(_) => {
                    self.rules[ri].filters[i] = Filter::IncludeLayover(self.entries[ri][i].clone())
                }
                Filter::ExcludeLayover(_) => {
                    self.rules[ri].filters[i] = Filter::ExcludeLayover(self.entries[ri][i].clone())
                }
                Filter::IncludeId(_) => {
                    self.rules[ri].filters[i] = Filter::IncludeId(self.entries[ri][i].clone())
                }
                Filter::IsPrem => {}
            },
            _ => {}
        }
    }

    fn view(&self) -> Element<Message> {
        /*
            pick_list for dropdowns
            checkbox for enabled
        */
        container(
            scrollable(
                column![
                    column(self.rules.iter().enumerate().map(|(i, r)| r.view(
                        i,
                        self.enabled[i],
                        &self.entries[i]
                    )))
                    .spacing(5),
                    container(
                        button(container("New Rule")
                            //.center_x(Length::Fill)
                        )
                        .on_press(Message::NewRule) //.width(Length::Fill)
                    )
                    .center_x(Length::Fill),
                    //.style(bordered_box),
                ]
                .spacing(5),
            )
            .spacing(5),
        )
        //.style(bordered_box)
        //.padding(5)
        .height(Length::FillPortion(9))
        .width(Length::Fill)
        .into()
    }
}

impl Rule {
    fn view(&self, index: usize, state: bool, entries: &[String]) -> Element<Message> {
        /*
            pick_list for dropdowns
            checkbox for enabled
        */
        let filters = [
            FilterType::TimeDiff,
            FilterType::FieldIs,
            FilterType::DateIs,
            FilterType::IncludeLayover,
            FilterType::ExcludeLayover,
            FilterType::NumDays,
            FilterType::IsPrem,
            FilterType::IncludeId,
        ];
        let actions = [BotAction::Ignore, BotAction::Pickup, BotAction::Alert];
        container(
            column![
                container(
                    row![
                        text(&self.name),
                        checkbox("Enable", state).on_toggle(move |b| if b {
                            Message::EnableRule(index)
                        } else {
                            Message::DisableRule(index)
                        }),
                        iced::widget::pick_list(actions, Some(self.action), move |a| {
                            Message::ChangeRuleAction(index, a)
                        }),
                        button("X").on_press(Message::DeleteRule(index))
                    ]
                    .spacing(10),
                )
                //.padding(Padding::from(10))
                .center_x(Length::Fill),
                column(
                    self.filters
                        .iter()
                        .enumerate()
                        .map(|(i, r)| r.view(index, i, &entries[i]))
                )
                .spacing(5),
                container(iced::widget::pick_list(
                    filters,
                    Some(FilterType::NewFilter),
                    move |f| { Message::NewFilter(index, f) }
                ))
                .center_x(Length::Fill),
            ]
            .spacing(5),
        )
        .style(bordered_box)
        .padding(Padding::from(5))
        .center_x(Length::Fill)
        .into()
    }
}

impl Filter {
    fn view(&self, ruleindex: usize, index: usize, entry: &str) -> Element<Message> {
        let fields = [
            Field::Report,
            Field::Depart,
            Field::Arrive,
            Field::Block,
            Field::Credit,
        ];
        let ops = [Op::Eq, Op::NEq, Op::Lt, Op::LtEq, Op::GtEq, Op::Gt];

        container(
            column![
                container(row![
                    text(self.as_string()),
                    container(button("Delete").on_press(Message::DeleteFilter(ruleindex, index)))
                        .align_right(Length::Fill)
                ]),
                match *self {
                    Filter::IsPrem => {
                        container(text("Premium only"))
                    }
                    Filter::TimeDiff(f1, f2, op, t) => {
                        container(row![
                            iced::widget::pick_list(fields, Some(f1), move |new_f1| {
                                Message::UpdateFilter(
                                    ruleindex,
                                    index,
                                    Filter::TimeDiff(new_f1, f2, op, t),
                                )
                            }),
                            iced::widget::pick_list(fields, Some(f2), move |new_f2| {
                                Message::UpdateFilter(
                                    ruleindex,
                                    index,
                                    Filter::TimeDiff(f1, new_f2, op, t),
                                )
                            }),
                            iced::widget::pick_list(ops, Some(op), move |new_op| {
                                Message::UpdateFilter(
                                    ruleindex,
                                    index,
                                    Filter::TimeDiff(f1, f2, new_op, t),
                                )
                            }),
                            iced::widget::text_input("time", &format!("{}", entry))
                                .on_input(move |new| Message::UpdateEntry(ruleindex, index, new))
                                .on_submit(Message::SubmitEntry(ruleindex, index, self.clone())),
                        ])
                    }
                    Filter::FieldIs(f, op, t) => {
                        container(row![
                            iced::widget::pick_list(fields, Some(f), move |new_f| {
                                Message::UpdateFilter(
                                    ruleindex,
                                    index,
                                    Filter::FieldIs(new_f, op, t),
                                )
                            }),
                            iced::widget::pick_list(ops, Some(op), move |new_op| {
                                Message::UpdateFilter(
                                    ruleindex,
                                    index,
                                    Filter::FieldIs(f, new_op, t),
                                )
                            }),
                            iced::widget::text_input("time", &format!("{}", entry))
                                .on_input(move |new| Message::UpdateEntry(ruleindex, index, new))
                                .on_submit(Message::SubmitEntry(ruleindex, index, self.clone())),
                        ])
                    }
                    Filter::DateIs(op, d) => {
                        container(row![
                            iced::widget::pick_list(ops, Some(op), move |new_op| {
                                Message::UpdateFilter(ruleindex, index, Filter::DateIs(new_op, d))
                            }),
                            iced::widget::text_input("date", &format!("{}", entry))
                                .on_input(move |new| Message::UpdateEntry(ruleindex, index, new))
                                .on_submit(Message::SubmitEntry(ruleindex, index, self.clone())),
                        ])
                    }
                    Filter::IncludeLayover(_) => {
                        container(row![iced::widget::text_input(
                            "Airport Code",
                            &format!("{}", entry)
                        )
                        .on_input(move |new| Message::UpdateEntry(ruleindex, index, new))
                        .on_submit(Message::SubmitEntry(ruleindex, index, self.clone())),])
                    }
                    Filter::ExcludeLayover(_) => {
                        container(row![iced::widget::text_input(
                            "Airport Code",
                            &format!("{}", entry)
                        )
                        .on_input(move |new| Message::UpdateEntry(ruleindex, index, new))
                        .on_submit(Message::SubmitEntry(ruleindex, index, self.clone())),])
                    }
                    Filter::NumDays(op, num) => {
                        container(row![
                            iced::widget::pick_list(ops, Some(op), move |new_op| {
                                Message::UpdateFilter(
                                    ruleindex,
                                    index,
                                    Filter::NumDays(new_op, num),
                                )
                            }),
                            iced::widget::text_input("Number of Days", &format!("{}", entry))
                                .on_input(move |new| Message::UpdateEntry(ruleindex, index, new))
                                .on_submit(Message::SubmitEntry(ruleindex, index, self.clone())),
                        ])
                    }
                    Filter::IncludeId(_) => {
                        container(row![iced::widget::text_input(
                            "Trip ID",
                            &format!("{}", entry)
                        )
                        .on_input(move |new| Message::UpdateEntry(ruleindex, index, new))
                        .on_submit(Message::SubmitEntry(ruleindex, index, self.clone())),])
                    }
                }
            ]
            .spacing(5),
        )
        .padding(Padding::from(5))
        .center_x(Length::Fill)
        .style(filter_box)
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
            _ => {} /*
                    m => {
                        self.log.push_str(&format!("{:?}\n", m));
                    }
                    */
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
