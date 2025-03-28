use crate::Message;
use clipboard_win::{formats, get_clipboard, set_clipboard};
use enigo::{
    Button, Coordinate,
    Direction::{Click, Press, Release},
    Enigo, Key, Keyboard, Mouse, Settings,
};
//use rand::Rng;
use regex::{Regex, RegexBuilder};
use serde::{Deserialize, Serialize};
use std::{
    cell::LazyCell,
    fmt::Display,
    fs::File,
    io::prelude::*,
    ops::Sub,
    str::FromStr,
    time::{Duration, Instant},
};

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct BotConfig {
    pub updated_time_pos: (u32, u32),
    pub refresh_interval: (u32, u32),
}

impl BotConfig {
    fn load() -> Result<BotConfig, ()> {
        let mut file = match File::open("config.json") {
            Ok(f) => f,
            Err(_) => {
                BotConfig::save_default();
                File::open("config.json")?
            }
        };

        let data;
        file.read_to_string(data)?;
        serde_json::from_str(data)
    }

    fn save_default() {
        let conf = BotConfig {
            updated_time_pos: (0, 0),
            refresh_interval: (30, 90),
        };

        let js: String = match serde_json::to_string(&conf) {
            Ok(s) => s,
            Err(_) => return,
        };

        let mut file = match File::create("config.json") {
            Ok(f) => f,
            Err(_) => return,
        };
        file.write_all(js.as_bytes())?;
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Rule {
    pub name: String,
    pub filters: Vec<Filter>,
    pub action: BotAction,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub enum Op {
    Eq,
    NEq,
    Lt,
    LtEq,
    GtEq,
    Gt,
}

impl Display for Op {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Op::Eq => "=",
                Op::NEq => "!=",
                Op::Lt => "<",
                Op::LtEq => "<=",
                Op::GtEq => ">=",
                Op::Gt => ">",
            }
        )
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub enum Field {
    Report,
    Depart,
    Arrive,
    Block,
    Credit,
}

impl Display for Field {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Field::Report => "Report",
                Field::Depart => "Depart",
                Field::Arrive => "Arrive",
                Field::Block => "Block",
                Field::Credit => "Credit",
            }
        )
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Filter {
    TimeDiff(Field, Field, Op, Time),
    FieldIs(Field, Op, Time),
    DateIs(Op, Date),
    IncludeLayover(String),
    ExcludeLayover(String),
    NumDays(Op, u8),
    IsPrem,
    IncludeId(String),
}

impl Filter {
    pub fn name(&self) -> &str {
        match self {
            Filter::TimeDiff(_, _, _, _) => "TimeDiff",
            Filter::FieldIs(_, _, _) => "FieldIs",
            Filter::DateIs(_, _) => "DateIs",
            Filter::IncludeLayover(_) => "IncludeLay",
            Filter::ExcludeLayover(_) => "ExcludeLay",
            Filter::NumDays(_, _) => "NumDays",
            Filter::IsPrem => "IsPrem",
            Filter::IncludeId(_) => "IsID",
        }
    }

    pub fn as_string(&self) -> String {
        match self {
            Filter::TimeDiff(lhs, rhs, op, t) => format!("{} - {} {} {}", lhs, rhs, op, t),
            Filter::FieldIs(f, op, t) => format!("{} {} {}", f, op, t),
            Filter::DateIs(op, d) => format!("Date {} {}", op, d),
            Filter::IncludeLayover(s) => format!("Include [{}]", s),
            Filter::ExcludeLayover(s) => format!("Exclude [{}]", s),
            Filter::NumDays(op, num) => format!("Days {} {}", op, num),
            Filter::IsPrem => "Is Premium".to_owned(),
            Filter::IncludeId(s) => format!("Trip ID is \"{}\"", s),
        }
    }

    pub fn eval(&self, trip: &Trip) -> bool {
        match self {
            Filter::TimeDiff(lhs, rhs, op, val) => match op {
                Op::Eq => trip.get(*lhs) - trip.get(*rhs) == *val,
                Op::NEq => trip.get(*lhs) - trip.get(*rhs) != *val,
                Op::Lt => trip.get(*lhs) - trip.get(*rhs) < *val,
                Op::LtEq => trip.get(*lhs) - trip.get(*rhs) <= *val,
                Op::Gt => trip.get(*lhs) - trip.get(*rhs) > *val,
                Op::GtEq => trip.get(*lhs) - trip.get(*rhs) >= *val,
            },
            Filter::FieldIs(field, op, val) => match op {
                Op::Eq => trip.get(*field) == *val,
                Op::NEq => trip.get(*field) != *val,
                Op::Lt => trip.get(*field) < *val,
                Op::LtEq => trip.get(*field) <= *val,
                Op::Gt => trip.get(*field) > *val,
                Op::GtEq => trip.get(*field) >= *val,
            },
            Filter::DateIs(op, val) => match op {
                Op::Eq => trip.date == *val,
                Op::NEq => trip.date != *val,
                Op::Lt => trip.date < *val,
                Op::LtEq => trip.date <= *val,
                Op::Gt => trip.date > *val,
                Op::GtEq => trip.date >= *val,
            },
            Filter::IncludeLayover(val) => trip.layovers.contains(val),
            Filter::ExcludeLayover(val) => !trip.layovers.contains(val),
            Filter::NumDays(op, val) => match op {
                Op::Eq => trip.days == *val,
                Op::NEq => trip.days != *val,
                Op::Lt => trip.days < *val,
                Op::LtEq => trip.days <= *val,
                Op::Gt => trip.days > *val,
                Op::GtEq => trip.days >= *val,
            },
            Filter::IsPrem => trip.premium,
            Filter::IncludeId(val) => trip.id == *val,
        }
    }
}

impl From<FilterType> for Filter {
    fn from(value: FilterType) -> Self {
        match value {
            FilterType::NewFilter => panic!("no filter selected"),
            FilterType::TimeDiff => {
                Filter::TimeDiff(Field::Report, Field::Report, Op::Eq, Time::default())
            }
            FilterType::FieldIs => Filter::FieldIs(Field::Report, Op::Eq, Time::default()),
            FilterType::DateIs => Filter::DateIs(Op::Eq, Date::default()),
            FilterType::IncludeLayover => Filter::IncludeLayover(String::new()),
            FilterType::ExcludeLayover => Filter::ExcludeLayover(String::new()),
            FilterType::NumDays => Filter::NumDays(Op::Eq, 1),
            FilterType::IsPrem => Filter::IsPrem,
            FilterType::IncludeId => Filter::IncludeLayover(String::new()),
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum FilterType {
    NewFilter,
    TimeDiff,
    FieldIs,
    DateIs,
    IncludeLayover,
    ExcludeLayover,
    NumDays,
    IsPrem,
    IncludeId,
}

impl Display for FilterType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                FilterType::NewFilter => "NewFilter",
                FilterType::TimeDiff => "TimeDiff",
                FilterType::FieldIs => "FieldIs",
                FilterType::DateIs => "DateIs",
                FilterType::IncludeLayover => "IncludeLay",
                FilterType::ExcludeLayover => "ExcludeLay",
                FilterType::NumDays => "NumDays",
                FilterType::IsPrem => "IsPrem",
                FilterType::IncludeId => "IsID",
            }
        )
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum BotAction {
    Alert,
    Pickup,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub struct Time {
    pub hours: u8,
    pub minutes: u8,
}

impl Default for Time {
    fn default() -> Self {
        Time {
            hours: 0,
            minutes: 0,
        }
    }
}

impl Time {
    pub fn from_num_str(s: &str) -> Result<Self, ParseTimeError> {
        if s.len() == 4 {
            Ok(Time {
                hours: s[0..2].parse().or(Err(ParseTimeError))?,
                minutes: s[2..4].parse().or(Err(ParseTimeError))?,
            })
        } else if s.len() == 5 {
            Ok(Time {
                hours: s[0..2].parse().or(Err(ParseTimeError))?,
                minutes: s[3..5].parse().or(Err(ParseTimeError))?,
            })
        } else {
            Err(ParseTimeError)
        }
    }
}

impl Sub for Time {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self {
        if rhs.minutes > self.minutes {
            Time {
                hours: if rhs.hours > self.hours {
                    self.hours - rhs.hours - 1
                } else {
                    12 + self.hours - rhs.hours - 1
                },
                minutes: 60 + self.minutes - rhs.minutes,
            }
        } else {
            Time {
                hours: if rhs.hours > self.hours {
                    self.hours - rhs.hours
                } else {
                    12 + self.hours - rhs.hours
                },
                minutes: self.minutes - rhs.minutes,
            }
        }
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct ParseTimeError;

impl FromStr for Time {
    type Err = ParseTimeError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.len() == 4 {
            Ok(Time {
                hours: s[0..2].parse().or(Err(ParseTimeError))?,
                minutes: s[2..4].parse().or(Err(ParseTimeError))?,
            })
        } else if s.len() == 5 {
            Ok(Time {
                hours: s[0..2].parse().or(Err(ParseTimeError))?,
                minutes: s[3..5].parse().or(Err(ParseTimeError))?,
            })
        } else {
            Err(ParseTimeError)
        }
    }
}

impl Display for Time {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:02.2}:{:02.2}", self.hours, self.minutes)
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub struct Date {
    pub year: u16,
    pub month: u8,
    pub day: u8,
}

impl Default for Date {
    fn default() -> Self {
        Date {
            year: 2025,
            month: 1,
            day: 1,
        }
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct ParseDateError;

impl FromStr for Date {
    type Err = ParseDateError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Date {
            year: 2024,
            month: month_from_str(&s[2..5])?,
            day: s[0..2].parse().or(Err(ParseDateError))?,
        })
    }
}

fn month_from_str(s: &str) -> Result<u8, ParseDateError> {
    match s {
        "JAN" => Ok(1),
        "FEB" => Ok(2),
        "MAR" => Ok(3),
        "APR" => Ok(4),
        "MAY" => Ok(5),
        "JUN" => Ok(6),
        "JUL" => Ok(7),
        "AUG" => Ok(8),
        "SEP" => Ok(9),
        "OCT" => Ok(10),
        "NOV" => Ok(11),
        "DEC" => Ok(12),
        _ => Err(ParseDateError),
    }
}

impl Display for Date {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {:02.2}, {:04.4}", self.month, self.day, self.year)
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Trip {
    id: String,
    date: Date,
    days: u8,
    report: Time,
    depart: Time,
    arrive: Time,
    block: Time,
    credit: Time,
    layovers: Vec<String>,
    premium: bool,
}

impl Trip {
    pub fn get(&self, field: Field) -> Time {
        match field {
            Field::Report => self.report,
            Field::Depart => self.depart,
            Field::Arrive => self.arrive,
            Field::Block => self.block,
            Field::Credit => self.credit,
        }
    }
}

pub async fn monitor_opentime() -> Message {
    let config: LazyCell<BotConfig> = LazyCell::new(|| BotConfig::load());
    let re_international: LazyCell<Regex> =
        LazyCell::new(|| Regex::new(r"DUB|EDI|LHR|LGW|CDG|AMS").unwrap());
    let re_opentime_trip: LazyCell<Regex> = LazyCell::new(|| {
        RegexBuilder::new(r"^(?P<tripid>\w+)\s+(?P<date>\w+)\s+(?P<days>\d+)\s+(?P<report>\S+)\s+(?P<depart>\S+)\s+(?P<arrive>\S+)\s+(?P<bulk>\d+)\s+(?P<credit>\d+)\s+(?P<layovers>(?:\S{3}\s*)*)\s*(?P<prem>X?)\s*$")
            .multi_line(true)
            .build()
            .unwrap()
    });
    let mut enigo = Enigo::new(&Settings::default()).unwrap();

    let loc_opentime = (500, 500);
    //let mut page_text = String::new();
    let mut last_refresh = Instant::now();
    let mut refresh_interval = Duration::from_secs(config.refresh_interval.0);

    let rule = Rule {
        name: "trips after 0700".to_owned(),
        filters: vec![Filter::FieldIs(
            Field::Report,
            Op::Gt,
            Time {
                hours: 7,
                minutes: 0,
            },
        )],
        action: BotAction::Alert,
    };

    async_std::task::sleep(Duration::from_secs(1)).await;

    loop {
        // click mouse to focus window
        let _ = enigo.move_mouse(loc_opentime.0, loc_opentime.1, Coordinate::Abs);
        let _ = enigo.button(Button::Left, Click);

        // refresh page
        if last_refresh.elapsed() > refresh_interval {
            refresh_page(&mut enigo, loc_opentime).await;
            last_refresh = Instant::now();
            refresh_interval = Duration::from_secs(rand::random_range(
                config.refresh_interval.0..config.refresh_interval.1,
            ));
            println!("refreshing and waiting {}", refresh_interval.as_secs());
        }

        // copy text
        let _ = enigo.key(Key::Control, Press);
        let _ = enigo.key(Key::Unicode('a'), Click);
        let _ = enigo.key(Key::Unicode('c'), Click);
        let _ = enigo.key(Key::Control, Release);
        async_std::task::sleep(Duration::from_millis(500)).await;

        // process text
        let result: String = get_clipboard(formats::Unicode).expect("To set clipboard");
        let trips: Vec<Trip> = re_opentime_trip
            .captures_iter(&result)
            .map(|c| c.extract())
            .map(
                |(_, [id, date, days, rep, dep, arr, blk, crd, lay, prem])| Trip {
                    id: id.to_owned(),
                    date: date.parse().unwrap(),
                    days: days.parse().unwrap(),
                    report: rep.parse().unwrap(),
                    depart: dep.parse().unwrap(),
                    arrive: arr.parse().unwrap(),
                    block: Time::from_num_str(blk).unwrap(),
                    credit: Time::from_num_str(crd).unwrap(),
                    layovers: lay.split_whitespace().map(|s| s.to_owned()).collect(),
                    premium: !prem.is_empty(),
                },
            )
            .collect();

        // apply filters
        let filtered_trips: Vec<&Trip> = trips
            .iter()
            .filter(|t| {
                rule.filters
                    .iter()
                    .map(|f| f.eval(t))
                    .fold(true, |a, b| a & b)
            })
            .collect();

        // alert if any match
        filtered_trips.iter().for_each(|t| println!("{}", t.id));

        // TEMP: add and submit a trip
        //add_trip_from_opentime(&mut enigo, "j2B15").await;
        //break;

        // sleep for a random ammount of time
        let milis_to_sleep = rand::random_range(800..2000);
        let mut m = 0;
        while m < milis_to_sleep {
            // check if Escape key is pressed

            if unsafe { winapi::um::winuser::GetKeyState(27) } & 0x8000u16 as i16 != 0 {
                return Message::Pause;
            }
            async_std::task::sleep(Duration::from_millis(50)).await;
            m += 50;
        }
    }

    //std::thread::sleep(Duration::from_secs(3)); // cant abort if this is used and there is no async sleep after it
    //Message::Pause
}

async fn refresh_page(enigo: &mut Enigo, loc: (i32, i32)) {
    // refresh page
    let _ = enigo.key(Key::Control, Press);
    let _ = enigo.key(Key::Unicode('r'), Click);
    let _ = enigo.key(Key::Control, Release);
    async_std::task::sleep(Duration::from_millis(500)).await;

    // click mouse in proper area
    let _ = enigo.move_mouse(loc.0, loc.1, Coordinate::Abs);
    let _ = enigo.button(Button::Left, Click);
}

async fn add_trip_from_otadd(enigo: &mut Enigo, trip_id: &str) {
    hit_button(enigo, trip_id).await;
    hit_button(enigo, "it r").await;
}

async fn add_trip_from_opentime(enigo: &mut Enigo, trip_id: &str) {
    hit_button(enigo, "submit").await;
    async_std::task::sleep(Duration::from_millis(2000)).await; // this delay needs to wait until the page has loaded
    hit_button(enigo, "add").await;
    //hit_button(enigo, trip_id).await;
    //hit_button(enigo, "it r").await;
}

// these durations should be randomized if possible, should total to ~1 sec
async fn hit_button(enigo: &mut Enigo, button_name: &str) {
    // open quick find bar
    let _ = enigo.key(Key::Unicode('/'), Click);
    async_std::task::sleep(Duration::from_millis(25)).await;

    // type button name
    let _ = enigo.text(button_name);
    async_std::task::sleep(Duration::from_millis(25)).await;

    // navigate to button
    let _ = enigo.key(Key::Shift, Press);
    let _ = enigo.key(Key::Tab, Click);
    let _ = enigo.key(Key::Shift, Release);
    async_std::task::sleep(Duration::from_millis(50)).await;

    // click button
    let _ = enigo.key(Key::Return, Click);
    async_std::task::sleep(Duration::from_millis(5)).await;
}
