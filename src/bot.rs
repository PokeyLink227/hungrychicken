use crate::{App, AppState, Message};
use enigo::{
    Button, Coordinate,
    Direction::{Click, Press, Release},
    Enigo, Key, Keyboard, Mouse, Settings,
};
use regex::{Regex, RegexBuilder};
use rodio::{Decoder, OutputStream, Sink, source::Source};
use serde::{Deserialize, Serialize};
use std::{
    cell::LazyCell,
    fmt::Display,
    fs::File,
    io::{BufReader, prelude::*},
    ops::Sub,
    str::FromStr,
    sync::mpsc::{Receiver, Sender},
    thread,
    time::{Duration, Instant},
};

#[derive(Debug, Clone)]
pub enum BotMessage {
    Start(Vec<Rule>),
    Stop,
    TripFound,
    Screenshot,
    TableMissing,
    CopyScreen,
    Waiting(u64),
    Copied(String),
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct BotConfig {
    pub updated_time_pos: (i32, i32, u32, u32),
    pub refresh_interval: (u32, u32),
    pub refresh: [u32; 4],
}

impl BotConfig {
    fn load() -> Result<BotConfig, ()> {
        let mut file = match File::open("config.json") {
            Ok(f) => f,
            Err(_) => {
                println!("config not found");
                BotConfig::save_default();
                File::open("config.json").or(Err(()))?
            }
        };

        let mut data = String::new();
        file.read_to_string(&mut data).or(Err(()))?;
        serde_json::from_str(&data).or(Err(()))
    }

    fn save_default() {
        let conf = BotConfig {
            updated_time_pos: (517, 179, 150, 40),
            refresh_interval: (10, 30),
            refresh: [87, 62, 20, 20],
        };

        let js: String = match serde_json::to_string(&conf) {
            Ok(s) => s,
            Err(_) => return,
        };

        let mut file = match File::create("config.json") {
            Ok(f) => f,
            Err(_) => return,
        };
        let _ = file.write_all(js.as_bytes());
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct Rule {
    pub name: String,
    pub filters: Vec<Filter>,
    pub action: BotAction,
}

impl Rule {
    pub fn eval(&self, trip: &Trip) -> bool {
        for filter in &self.filters {
            if !filter.eval(trip) {
                println!("trip {} failed filter {:?}", trip.id, filter);
                return false;
            }
        }

        true
    }

    pub fn get_action(&self, trip: &Trip) -> BotAction {
        if self.eval(trip) {
            self.action
        } else {
            BotAction::Nothing
        }
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
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

#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
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

#[derive(Debug, Copy, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[repr(u8)]
pub enum BotAction {
    Nothing = 1,
    Alert = 2,
    Pickup = 3,
    Ignore = 4,
}

impl Display for BotAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                BotAction::Nothing => "Do Nothing",
                BotAction::Alert => "Alert",
                BotAction::Pickup => "Pickup",
                BotAction::Ignore => "Ignore",
            }
        )
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
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

#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
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
        if s.len() != 5 {
            return Err(ParseDateError);
        }
        Ok(Date {
            year: 2025,
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
        write!(
            f,
            "{} {:02.2}, {:04.4}",
            match self.month {
                1 => "JAN",
                2 => "FEB",
                3 => "MAR",
                4 => "APR",
                5 => "MAY",
                6 => "JUN",
                7 => "JUL",
                8 => "AUG",
                9 => "SEP",
                10 => "OCT",
                11 => "NOV",
                12 => "DEC",
                _ => "N/A",
            },
            self.day,
            self.year
        )
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

use std::collections::HashSet;

use xcap::image::{Rgba, RgbaImage};

const CAP_LEN: usize = 5;
const TABLE_LINE_WIDTH: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpentimeRow {
    pub y: u32,
    pub h: u32,

    pub x_pairing: u32,
    pub w_pairing: u32,

    pub x_premium: u32,
}

// TODO: validate this is the first row using the blue header
// for pairing and premium rows
fn table_moved(screen: &RgbaImage, first_row: &OpentimeRow) -> bool {
    let white = Rgba::<u8>::from([0xFF, 0xFF, 0xFF, 0xFF]);
    let light_gray = Rgba::<u8>::from([0xB2, 0xB2, 0xB2, 0xFF]);
    let dark_gray = Rgba::<u8>::from([0x4C, 0x4C, 0x4C, 0xFF]);
    let light_blue = Rgba::<u8>::from([0xB0, 0xC4, 0xDE, 0xFF]);

    if first_row.h == 0 {
        return true;
    } else if first_row.y < 3 {
        return true;
    }

    *screen.get_pixel(first_row.x_pairing, first_row.y) != white
        || *screen.get_pixel(first_row.x_pairing, first_row.y - 1) != dark_gray
        || *screen.get_pixel(first_row.x_pairing, first_row.y - 2) != light_gray
        || *screen.get_pixel(first_row.x_pairing, first_row.y - 3) != light_blue
}

fn find_first_row(screen: &RgbaImage) -> Option<OpentimeRow> {
    let (width, height) = screen.dimensions();
    println!("Finding first row");

    let white = Rgba::<u8>::from([0xFF, 0xFF, 0xFF, 0xFF]);
    let light_gray = Rgba::<u8>::from([0xB2, 0xB2, 0xB2, 0xFF]);
    let dark_gray = Rgba::<u8>::from([0x4C, 0x4C, 0x4C, 0xFF]);
    let light_blue = Rgba::<u8>::from([0xB0, 0xC4, 0xDE, 0xFF]);

    let mut table_top_left_inner_corner = None;
    let mut table_top_right_inner_corner = None;

    let mut trip_id_pos = None;

    let mut x = 0;
    let mut y = 0;

    // Find left bounds of table
    'outer: while y < height - 8 {
        while x < width - 8 {
            if *screen.get_pixel(x, y) == light_gray
                && *screen.get_pixel(x + TABLE_LINE_WIDTH, y + TABLE_LINE_WIDTH) == dark_gray
                && *screen.get_pixel(x + TABLE_LINE_WIDTH * 2, y + TABLE_LINE_WIDTH * 2)
                    == light_gray
                && *screen.get_pixel(x + TABLE_LINE_WIDTH * 3, y + TABLE_LINE_WIDTH * 3)
                    == dark_gray
                && *screen.get_pixel(x + TABLE_LINE_WIDTH * 4, y + TABLE_LINE_WIDTH * 4)
                    == light_blue
            {
                table_top_left_inner_corner =
                    Some((x + TABLE_LINE_WIDTH * 2, y + TABLE_LINE_WIDTH * 2));
                break 'outer;
            }
            x += 1;
        }
        y += 1;
        x = 0;
    }
    let table_top_left_inner_corner = table_top_left_inner_corner?;
    println!("{:?}", table_top_left_inner_corner);

    // Find right bounds of table
    x += TABLE_LINE_WIDTH * 2;
    y += TABLE_LINE_WIDTH * 2;
    while x < width - 2 {
        if *screen.get_pixel(x, y) != light_gray
            && *screen.get_pixel(x - TABLE_LINE_WIDTH * 2, y + TABLE_LINE_WIDTH * 2) == light_blue
        {
            table_top_right_inner_corner = Some((x, y));
            break;
        }
        x += 1;
    }
    let table_top_right_inner_corner = table_top_right_inner_corner?;
    println!("{:?}", table_top_right_inner_corner);

    // Find first row

    // Get bounds of Pairing column
    (x, y) = table_top_left_inner_corner;
    while y < height {
        if *screen.get_pixel(x, y) == white {
            trip_id_pos = Some((x, y));
            break;
        }
        y += 1;
    }
    let trip_id_pos = trip_id_pos?;

    while y < height {
        if *screen.get_pixel(x, y) != white {
            break;
        }
        y += 1;
    }
    while x < width {
        if *screen.get_pixel(x, y - 1) != white {
            break;
        }
        x += 1;
    }
    let trip_id_size = (x - trip_id_pos.0, y - trip_id_pos.1);
    println!("{:?}", trip_id_pos);
    println!("{:?}", trip_id_size);

    // TODO: only compare white pixels to avoid clicked (purple) links being different

    // Get bounds of Premium column
    x = table_top_right_inner_corner.0;
    y = trip_id_pos.1;
    while x > trip_id_pos.0 {
        if *screen.get_pixel(x, y) != white {
            break;
        }
        x -= 1;
    }
    let prem_mid_pos = ((table_top_right_inner_corner.0 + x) / 2, trip_id_pos.1);
    let prem_mid_size = (1, trip_id_size.1);
    println!("{:?}", prem_mid_pos);
    println!("{:?}", prem_mid_size);

    let dim = OpentimeRow {
        y: trip_id_pos.1,
        h: trip_id_size.1,
        x_pairing: trip_id_pos.0,
        w_pairing: trip_id_size.0,
        x_premium: prem_mid_pos.0,
    };
    println!("{:?}", dim);

    save_row(screen, &dim, 999);

    Some(dim)
}

// convert to tiny vec? reuse return buffer?
fn collect_prem(screen: &RgbaImage, first_row: &OpentimeRow) -> Vec<OpentimeRow> {
    let mut prem_rows = Vec::new();

    'outer: for off_y in 0..CAP_LEN {
        let row_start: u32 = first_row.y + (first_row.h + TABLE_LINE_WIDTH * 2) * off_y as u32;
        if row_start + first_row.h >= screen.height() {
            break;
        }

        for y in 0..first_row.h {
            if *screen.get_pixel(first_row.x_premium, y + row_start)
                != Rgba::<u8>::from([0xFF, 0xFF, 0xFF, 0xFF])
            {
                prem_rows.push(OpentimeRow {
                    y: row_start,
                    ..*first_row
                });
                continue 'outer;
            }
        }
    }

    println!("PREM_ROWS: {:?}", prem_rows);
    prem_rows
}

fn save_row(screen: &RgbaImage, row: &OpentimeRow, id: usize) {
    let mut out_img = RgbaImage::new(row.w_pairing + 1, row.h);

    for y in 0..row.h {
        for x in 0..row.w_pairing {
            out_img[(x, y)] = screen[(x + row.x_pairing, y + row.y)];
        }
    }

    for y in 0..row.h {
        out_img[(row.w_pairing, y)] = screen[(row.x_premium, y + row.y)];
    }

    out_img.save(format!("row-{id}.png"));
}

fn is_ignored(screen: &RgbaImage, row: &OpentimeRow, ignored_trips: &HashSet<RgbaImage>) -> bool {
    let mut pairing_id = RgbaImage::new(row.w_pairing, row.h);
    for ty in 0..row.h {
        for tx in 0..row.w_pairing {
            pairing_id[(tx, ty)] = screen[(tx + row.x_pairing, ty + row.y)];
        }
    }

    ignored_trips.contains(&pairing_id)
}

fn ignore_trip(screen: &RgbaImage, row: &OpentimeRow, ignored_trips: &mut HashSet<RgbaImage>) {
    let mut pairing_id = RgbaImage::new(row.w_pairing, row.h);
    for ty in 0..row.h {
        for tx in 0..row.w_pairing {
            pairing_id[(tx, ty)] = screen[(tx + row.x_pairing, ty + row.y)];
        }
    }

    ignored_trips.insert(pairing_id);
}

fn bot_main(ignored_trips: &mut HashSet<RgbaImage>) -> Option<Vec<OpentimeRow>> {
    let screen = xcap::Monitor::all().unwrap()[0].clone();
    let cap = screen.capture_image().unwrap();

    let mut first_row: OpentimeRow = find_first_row(&cap)?;

    loop {
        if table_moved(&cap, &first_row) {
            first_row = find_first_row(&cap)?;
        }

        let prem_trips = collect_prem(&cap, &first_row);
        if prem_trips.is_empty() {
            // continue
            None?;
        } else {
            let new_prem: Vec<OpentimeRow> = prem_trips
                .into_iter()
                .filter(|trip| !is_ignored(&cap, trip, ignored_trips))
                .collect();
            if new_prem.is_empty() {
                // continue
                None?;
            } else {
                new_prem
                    .iter()
                    .for_each(|row| ignore_trip(&cap, row, ignored_trips));
                return Some(new_prem);
            }
        }
    }
}

pub fn image_bot_thread(rx: Receiver<BotMessage>, tx: Sender<BotMessage>) {
    let mut state = AppState::Stopped;
    let (_stream, stream_handle) = OutputStream::try_default().unwrap();
    let file = BufReader::new(File::open("alert_sound.wav").unwrap());
    let source = Decoder::new(file).unwrap();
    let sink = Sink::try_new(&stream_handle).unwrap();
    sink.append(source.repeat_infinite());
    sink.pause();

    let config: BotConfig = BotConfig::load().unwrap();

    let mut enigo = Enigo::new(&Settings::default()).unwrap();

    let mut missing_count = 0;
    let mut last_refresh = Instant::now();
    let mut refresh_interval = Duration::from_secs(config.refresh_interval.0 as u64);
    thread::sleep(Duration::from_secs(1));

    let mut ignored_trips = HashSet::<RgbaImage>::new();
    let screen = xcap::Monitor::all().unwrap()[0].clone();

    let mut first_row = OpentimeRow {
        y: 0,
        h: 0,
        x_pairing: 0,
        w_pairing: 0,
        x_premium: 0,
    };

    println!("bot entering main loop");
    'main: loop {
        if let Ok(msg) = rx.try_recv() {
            match msg {
                BotMessage::Start(r) => {
                    state = AppState::Running;
                    // rules = r;
                }
                BotMessage::Stop => {
                    state = AppState::Stopped;
                    sink.pause();
                }
                _ => {}
            }
        }

        if state != AppState::Running {
            thread::sleep(Duration::from_millis(100));
            continue 'main;
        }

        // refresh page
        if last_refresh.elapsed() > refresh_interval {
            last_refresh = Instant::now();
            refresh_interval = Duration::from_secs(rand::random_range(
                config.refresh_interval.0..config.refresh_interval.1,
            ) as u64);

            tx.send(BotMessage::Waiting(refresh_interval.as_secs()))
                .unwrap();

            // refresh page
            // let _ = enigo.key(Key::F5, Click);
            let _ = enigo.key(Key::Control, Press);
            let _ = enigo.key(Key::Unicode('r'), Click);
            let _ = enigo.key(Key::Control, Release);
        }

        thread::sleep(Duration::from_millis(1000));
        let cap = screen.capture_image().unwrap();
        // tx.send(BotMessage::Screenshot);

        if table_moved(&cap, &first_row) {
            let Some(new_first_row) = find_first_row(&cap) else {
                missing_count += 1;
                if missing_count > 5 {
                    state = AppState::Stopped;
                    tx.send(BotMessage::Stop);
                }
                tx.send(BotMessage::TableMissing);

                println!("First row missing");
                thread::sleep(Duration::from_millis(500));
                continue 'main;
            };

            missing_count = 0;
            first_row = new_first_row;
        }

        let new_prem: Vec<OpentimeRow> = collect_prem(&cap, &first_row)
            .into_iter()
            .filter(|trip| !is_ignored(&cap, trip, &ignored_trips))
            .collect();

        if new_prem.is_empty() {
            // continue
        } else {
            new_prem
                .iter()
                .for_each(|row| ignore_trip(&cap, row, &mut ignored_trips));

            // for debugging
            new_prem
                .iter()
                .enumerate()
                .for_each(|(idx, row)| save_row(&cap, row, idx));

            // Alert user
            sink.play();
            state = AppState::Alerting;
            tx.send(BotMessage::TripFound).unwrap();
        }
    }
}

pub fn bot_thread(rx: Receiver<BotMessage>, tx: Sender<BotMessage>) {
    let mut rules: Vec<Rule> = Vec::new();
    let mut state = AppState::Stopped;
    let (_stream, stream_handle) = OutputStream::try_default().unwrap();
    let file = BufReader::new(File::open("alert_sound.wav").unwrap());
    let source = Decoder::new(file).unwrap();
    let sink = Sink::try_new(&stream_handle).unwrap();
    sink.append(source.repeat_infinite());
    sink.pause();

    let config: BotConfig = BotConfig::load().unwrap();
    let re_international: Regex = Regex::new(r"DUB|EDI|LHR|LGW|CDG|AMS").unwrap();
    let re_opentime_trip: Regex = RegexBuilder::new(r"^(?P<tripid>\w+)\s+(?P<date>\w+)\s+(?P<days>\d+)\s+(?P<report>\S+)\s+(?P<depart>\S+)\s+(?P<arrive>\S+)\s+(?P<bulk>\d+)\s+(?P<credit>\d+)\s+(?P<layovers>(?:\S{3}\s*)*)\s*(?P<prem>X?)\s*$")
            .multi_line(true)
            .build()
            .unwrap()
    ;
    let mut enigo = Enigo::new(&Settings::default()).unwrap();
    let screen = screenshots::Screen::all().unwrap()[0];
    let mut image_update_time: screenshots::image::RgbaImage = screen
        .capture_area(
            config.updated_time_pos.0,
            config.updated_time_pos.1,
            config.updated_time_pos.2,
            config.updated_time_pos.3,
        )
        .unwrap();
    let mut new_update_time: screenshots::image::RgbaImage;
    let blank = screenshots::image::RgbaImage::from_pixel(
        config.updated_time_pos.2,
        config.updated_time_pos.3,
        screenshots::image::Rgba([255, 255, 255, 255]),
    );
    //image_update_time.save(format!("time.png")).unwrap();

    let loc_opentime = (500, 500);
    //let mut page_text = String::new();
    let mut last_refresh = Instant::now();
    let mut refresh_interval = Duration::from_secs(config.refresh_interval.0 as u64);
    thread::sleep(Duration::from_secs(1));

    // click mouse to focus window
    let _ = enigo.move_mouse(loc_opentime.0, loc_opentime.1, Coordinate::Abs);
    let _ = enigo.button(Button::Left, Click);
    thread::sleep(Duration::from_secs(1));

    let mut load_icon = screen
        .capture_area(
            config.refresh[0] as i32,
            config.refresh[1] as i32,
            config.refresh[2],
            config.refresh[3],
        )
        .unwrap();

    println!("bot entering main loop");
    'main: loop {
        if let Ok(msg) = rx.try_recv() {
            match msg {
                BotMessage::Start(r) => {
                    state = AppState::Running;
                    rules = r;
                    let _ = enigo.move_mouse(loc_opentime.0, loc_opentime.1, Coordinate::Abs);
                    let _ = enigo.button(Button::Left, Click);
                    thread::sleep(Duration::from_secs(1));
                    load_icon = screen
                        .capture_area(
                            config.refresh[0] as i32,
                            config.refresh[1] as i32,
                            config.refresh[2],
                            config.refresh[3],
                        )
                        .unwrap();
                }
                BotMessage::Stop => {
                    state = AppState::Stopped;
                    sink.pause();
                }
                _ => {}
            }
        }

        if state != AppState::Running {
            thread::sleep(Duration::from_millis(100));
            continue 'main;
        }
        // assume the browser window is still focused

        // refresh page
        if last_refresh.elapsed() > refresh_interval {
            last_refresh = Instant::now();
            refresh_interval = Duration::from_secs(rand::random_range(
                config.refresh_interval.0..config.refresh_interval.1,
            ) as u64);
            //println!("refreshing and waiting {}", refresh_interval.as_secs());
            tx.send(BotMessage::Waiting(refresh_interval.as_secs()))
                .unwrap();
            // refresh page
            let _ = enigo.key(Key::Control, Press);
            let _ = enigo.key(Key::Unicode('r'), Click);
            let _ = enigo.key(Key::Control, Release);

            // wait for page to finish loading
            while screen
                .capture_area(
                    config.refresh[0] as i32,
                    config.refresh[1] as i32,
                    config.refresh[2],
                    config.refresh[3],
                )
                .unwrap()
                != load_icon
            {
                thread::sleep(Duration::from_millis(100));
            }
            thread::sleep(Duration::from_millis(300));

            new_update_time = screen
                .capture_area(
                    config.updated_time_pos.0,
                    config.updated_time_pos.1,
                    config.updated_time_pos.2,
                    config.updated_time_pos.3,
                )
                .unwrap();
            while new_update_time.pixels().eq(blank.pixels()) {
                new_update_time = screen
                    .capture_area(
                        config.updated_time_pos.0,
                        config.updated_time_pos.1,
                        config.updated_time_pos.2,
                        config.updated_time_pos.3,
                    )
                    .unwrap();
                thread::sleep(Duration::from_millis(50));
            }
            thread::sleep(Duration::from_millis(500));

            // click mouse in proper area
            let _ = enigo.move_mouse(loc_opentime.0, loc_opentime.1, Coordinate::Abs);
            let _ = enigo.button(Button::Left, Click);
            thread::sleep(Duration::from_millis(300));
        }

        // take screencap to determine if page has changed
        // TODO: compare to blank image to ensure page has finished loading

        //println!("checking time");
        new_update_time = screen
            .capture_area(
                config.updated_time_pos.0,
                config.updated_time_pos.1,
                config.updated_time_pos.2,
                config.updated_time_pos.3,
            )
            .unwrap();

        if !new_update_time.pixels().eq(image_update_time.pixels()) {
            image_update_time.save("old.png");
            new_update_time.save("new.png");

            image_update_time = new_update_time;

            println!("Copying screen");
            tx.send(BotMessage::CopyScreen).unwrap();
            // copy text
            let _ = enigo.key(Key::Control, Press);
            let _ = enigo.key(Key::Unicode('a'), Click);
            let _ = enigo.key(Key::Unicode('c'), Click);
            let _ = enigo.key(Key::Control, Release);
            //let _ = enigo.key(Key::Tab, Click);
            thread::sleep(Duration::from_millis(150));
            let _ = enigo.button(Button::Left, Click);
            thread::sleep(Duration::from_millis(150));

            // process text
            let result = String::from("asd");
            tx.send(BotMessage::Copied(result.clone())).unwrap();
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
            let filtered_trips: Vec<(BotAction, &str)> =
                trips
                    .iter()
                    .map(|t| {
                        (
                            rules.iter().map(|r| r.get_action(t)).fold(
                                BotAction::Nothing,
                                |a, b| if b as u8 > a as u8 { b } else { a },
                            ),
                            t.id.as_str(),
                        )
                    })
                    .collect();

            // alert if any match
            for t in &filtered_trips {
                println!("{:?} {}", t.0, t.1);
                if t.0 == BotAction::Pickup {
                    add_trip_from_opentime(&mut enigo, t.1);
                    sink.play();
                    state = AppState::Stopped;
                    tx.send(BotMessage::Stop).unwrap();
                    continue;
                } else if t.0 == BotAction::Alert {
                    // alert user
                    sink.play();
                    state = AppState::Alerting;
                    tx.send(BotMessage::TripFound).unwrap();
                }
            }
        }

        //println!("sleeping");
        // sleep for a random ammount of time
        let milis_to_sleep = rand::random_range(150..250);
        let mut m = 0;
        while m < milis_to_sleep {
            // check if Escape key is pressed

            // if unsafe { winapi::um::winuser::GetKeyState(27) } & 0x8000u16 as i16 != 0 {
            //     println!("stopping");
            //     state = AppState::Stopped;
            //     tx.send(BotMessage::Stop).unwrap();
            //     continue 'main;
            // }
            thread::sleep(Duration::from_millis(50));
            m += 50;
        }
    }
}

fn add_trip_from_otadd(enigo: &mut Enigo, trip_id: &str) -> enigo::InputResult<()> {
    hit_button(enigo, trip_id)?;
    hit_button(enigo, "it r")
}

fn add_trip_from_opentime(enigo: &mut Enigo, trip_id: &str) -> enigo::InputResult<()> {
    hit_button(enigo, "submit")?;
    thread::sleep(Duration::from_millis(1500)); // this delay needs to wait until the page has loaded
    hit_button(enigo, "add")?;
    thread::sleep(Duration::from_millis(1500)); // this delay needs to wait until the page has loaded
    hit_button(enigo, trip_id)?;
    thread::sleep(Duration::from_millis(50));
    hit_button(enigo, "it r")
}

// these durations should be randomized if possible, should total to ~1 sec
fn hit_button(enigo: &mut Enigo, button_name: &str) -> enigo::InputResult<()> {
    println!("hitting [{}] button", button_name);

    // open quick find bar
    println!("hitting /");
    enigo.key(Key::Unicode('/'), Click)?;
    thread::sleep(Duration::from_millis(28));

    // type button name
    println!("hitting trip id");
    enigo.text(button_name)?;
    thread::sleep(Duration::from_millis(200));

    // navigate to button
    println!("hitting shoft+tab");
    //let _ = enigo.key(Key::Tab, Click);
    enigo.key(Key::Shift, Press)?;
    enigo.key(Key::Tab, Click)?;
    enigo.key(Key::Shift, Release)?;
    thread::sleep(Duration::from_millis(75));

    // click button
    println!("hitting enter");
    enigo.key(Key::Return, Click)?;
    thread::sleep(Duration::from_millis(5));

    Ok(())
}
