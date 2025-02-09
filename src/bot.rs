use crate::Message;
use clipboard_win::{formats, get_clipboard, set_clipboard};
use enigo::{
    Button, Coordinate,
    Direction::{Click, Press, Release},
    Enigo, Key, Keyboard, Mouse, Settings,
};
use rand::Rng;
use regex::{Regex, RegexBuilder};
use std::{
    cell::LazyCell,
    str::FromStr,
    time::{Duration, Instant},
};

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Rule {
    filters: Vec<Filter>,
    action: BotAction,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum Filter {
    NumDays(u8, u8),
    OnDay(Date),
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum BotAction {
    Alert,
    AutoPickup,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct Time {
    hours: u8,
    minutes: u8,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct ParseTimeError;

impl FromStr for Time {
    type Err = ParseTimeError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Time {
            hours: s[0..2].parse().or(Err(ParseTimeError))?,
            minutes: s[3..4].parse().or(Err(ParseTimeError))?,
        })
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct Date {
    year: u16,
    month: u8,
    day: u8,
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

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Trip {
    id: String,
    date: Date,
    days: u8,
    report: Time,
    depart: Time,
    arrive: Time,
    block: u16,
    credit: u16,
    layovers: Vec<String>,
    premium: bool,
}

pub async fn monitor_opentime() -> Message {
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
    let mut page_text = String::new();
    let mut last_refresh = Instant::now();
    let mut refresh_interval = Duration::from_secs(20);

    let rule = Rule {
        filters: vec![Filter::NumDays(3, 3)],
        action: BotAction::Alert,
    };

    async_std::task::sleep(Duration::from_secs(1)).await;

    loop {
        // click mouse to focus window
        enigo.move_mouse(loc_opentime.0, loc_opentime.1, Coordinate::Abs);
        enigo.button(Button::Left, Click);

        // refresh page
        if last_refresh.elapsed() > refresh_interval {
            refresh_page(&mut enigo, loc_opentime).await;
            last_refresh = Instant::now();
            refresh_interval = Duration::from_secs(rand::random_range(30..60));
            println!("refreshing and waiting {}", refresh_interval.as_secs());
        }

        // copy text
        enigo.key(Key::Control, Press);
        enigo.key(Key::Unicode('a'), Click);
        enigo.key(Key::Unicode('c'), Click);
        enigo.key(Key::Control, Release);
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
                    block: blk.parse().unwrap(),
                    credit: crd.parse().unwrap(),
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
                    .map(|f| match f {
                        Filter::NumDays(l, r) => t.days >= *l && t.days <= *r,
                        Filter::OnDay(d) => t.date == *d,
                    })
                    .fold(true, |a, b| a & b)
            })
            .collect();

        // alert if any match
        filtered_trips.iter().for_each(|t| println!("{}", t.id));

        // TEMP: add and submit a trip
        //add_trip_from_otadd(&mut enigo, "j2B15").await;

        // sleep for a random ammount of time
        let milis_to_sleep = rand::random_range(800..2000);
        let mut m = 0;
        while m < milis_to_sleep {
            // check if Escape key is pressed
            if unsafe { winapi::um::winuser::GetKeyState(27) } & 0x8000u16 as i16 != 0 {
                return Message::Pause;
            }
            async_std::task::sleep(Duration::from_millis(50)).await;
        }
    }

    //std::thread::sleep(Duration::from_secs(3)); // cant abort if this is used and there is no async sleep after it
    Message::Pause
}

async fn refresh_page(enigo: &mut Enigo, loc: (i32, i32)) {
    // refresh page
    enigo.key(Key::Control, Press);
    enigo.key(Key::Unicode('r'), Click);
    enigo.key(Key::Control, Release);
    async_std::task::sleep(Duration::from_millis(500)).await;

    // click mouse in proper area
    enigo.move_mouse(loc.0, loc.1, Coordinate::Abs);
    enigo.button(Button::Left, Click);
}

async fn add_trip_from_otadd(enigo: &mut Enigo, trip_id: &str) {
    // open quick find bar
    enigo.key(Key::Unicode('/'), Click);
    async_std::task::sleep(Duration::from_millis(100)).await;

    // type trip id
    enigo.text(trip_id);

    // navigate to add button
    enigo.key(Key::Shift, Press);
    enigo.key(Key::Tab, Click);
    enigo.key(Key::Shift, Release);

    // click add button
    enigo.key(Key::Return, Click);
}
