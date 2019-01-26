#![feature(proc_macro_hygiene, decl_macro)]

#[macro_use(lazy_static)]
extern crate lazy_static;
#[macro_use]
extern crate rocket;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;
#[macro_use(xplane_plugin)]
extern crate xplm;

use rocket::config::{Config, Environment};
use std::sync::mpsc;
use std::sync::Mutex;
use std::thread;
use xplm::data::borrowed::DataRef;
use xplm::data::DataRead;
use xplm::plugin::Plugin as XPlanePlugin;

macro_rules! gauge {
    ($str:expr) => {
        concat!("sim/cockpit2/gauges/indicators/", $str)
    }
}

const AIRSPEED: &'static str = gauge!("airspeed_kts_pilot");
const ALTITUDE: &'static str = gauge!("altitude_ft_pilot");
const HEADING: &'static str = gauge!("heading_vacuum_deg_mag_pilot");
const VERTICAL_SPEED: &'static str = gauge!("vvi_fpm_pilot");
const TURN: &'static str = gauge!("turn_rate_roll_deg_pilot");
const SLIP: &'static str = gauge!("slip_deg");

macro_rules! engine {
    ($str:expr) => {
        concat!("sim/cockpit2/engine/indicators/", $str)
    }
}

const MANIFOLD_PRESSURE: &'static str = engine!("MPR_in_hg");
const FUEL_FLOW: &'static str = engine!("fuel_flow_kg_sec");
const EXHAUST_TEMPERATURE: &'static str = engine!("EGT_deg_C");
const PROPELLER_SPEED: &'static str = engine!("prop_speed_rpm");

pub struct QuitMessage;

pub struct Plugin {
    sender: mpsc::Sender<QuitMessage>,
}

#[derive(Debug)]
pub struct PluginError;
impl std::error::Error for PluginError {}
impl std::fmt::Display for PluginError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{:?}", ())
    }
}

#[derive(Copy, Clone, Debug, Default, Serialize)]
pub struct Data {
    pub airspeed: f32,
    pub altitude: f32,
    pub heading: f32,
    pub vertical_speed: f32,
    pub turn: f32,
    pub slip: f32,

    pub rpm: f32,
    pub fuel_flow: f32,
    pub manifold_pressure: f32,
    pub egt: f32,
}

fn find(name: &str) -> f32 {
    DataRef::find(name)
        .ok()
        .map_or(0.0, |data| data.get())
}

fn poll() -> Data {
    Data {
        airspeed: find(AIRSPEED),
        altitude: find(ALTITUDE),
        heading: find(HEADING),
        vertical_speed: find(VERTICAL_SPEED),
        turn: find(TURN),
        slip: find(SLIP),
        rpm: find(PROPELLER_SPEED),
        fuel_flow: find(FUEL_FLOW),
        manifold_pressure: find(MANIFOLD_PRESSURE),
        egt: find(EXHAUST_TEMPERATURE),
    }
}

xplane_plugin!(Plugin);

impl xplm::plugin::Plugin for Plugin {
    type StartErr = PluginError;

    fn enable(&mut self) {
        let (sender, receiver) = mpsc::channel();
        self.sender = sender;
        thread::spawn(move || {
            loop {
                match receiver.try_recv() {
                    Ok(_) | Err(mpsc::TryRecvError::Disconnected) => break,
                    Err(mpsc::TryRecvError::Empty) => *DATA.lock().unwrap() = poll(),
                }
            }
        });
    }

    fn disable(&mut self) {
        // nop
    }

    fn stop(&mut self) {
        // nop
    }

    fn start() -> Result<Self, Self::StartErr> {
        thread::spawn(|| {
            let config = Config::build(Environment::Production)
                .address("0.0.0.0")
                .port(8000)
                .workers(1)
                .unwrap();
            rocket::custom(config)
                .mount("/", routes![get])
                .launch();
        });
        let (dummy_sender, _) = mpsc::channel();
        Ok(Plugin { sender: dummy_sender })
    }

    fn info(&self) -> xplm::plugin::PluginInfo {
        xplm::plugin::PluginInfo {
            name: "alteous-instruments".into(),
            signature: "alteous".into(),
            description: "A plugin written in Rust".into(),
        }
    }
}

lazy_static! {
    pub static ref DATA: Mutex<Data> = Mutex::new(Default::default());
}

#[get("/")]
fn get() -> String {
    let data = DATA.lock().unwrap().clone();
    serde_json::to_string(&data).unwrap()
}
