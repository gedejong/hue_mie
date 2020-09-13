extern crate dirs;
extern crate toml;

use philipshue::errors::{BridgeError, HueError, HueErrorKind};
use std::boxed::Box;
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {
    #[serde(default)]
    pub hue: Option<HueConfig>,

    #[serde(default)]
    pub location: Location,

    #[serde(default)]
    pub transitions: Transitions,
}

use std::fs::File;
use std::io::Read;

use philipshue::bridge;
use std::thread;
use std::time::Duration;

//#[cfg(feature = "upnp")]
pub fn discover() -> Vec<String> {
    let mut ips = bridge::discover_upnp().unwrap();
    ips.dedup();
    ips
}

#[cfg(all(feature = "nupnp", not(feature = "upnp")))]
pub fn discover() -> Vec<String> {
    use philipshue::hue::Discovery;
    Bridge::discover()
        .unwrap()
        .into_iter()
        .map(Discovery::into_ip)
        .collect()
}

/*
#[cfg(all(not(feature = "nupnp"), not(feature = "upnp")))]
pub fn discover() -> Vec<String> {
    panic!("Either UPnP or NUPnP is required for discovering!")
}
*/

impl Config {
    pub fn get_hue_config() -> Result<HueConfig, Box<dyn std::error::Error>> {
        let ip: String = discover().pop().unwrap();

        loop {
            match bridge::register_user(&ip, "hue_cycle") {
                Ok(bridge) => {
                    println!("User registered: {}, on IP: {}", bridge, ip);
                    return Ok(HueConfig {
                        bridge_ip: ip,
                        bridge_password: bridge,
                    });
                }
                Err(HueError(
                    HueErrorKind::BridgeError {
                        error: BridgeError::LinkButtonNotPressed,
                        ..
                    },
                    _,
                )) => {
                    println!("Please, press the link on the bridge. Retrying in 5 seconds");
                    thread::sleep(Duration::from_secs(5));
                }
                Err(e) => {
                    return Err(Box::new(e));
                }
            }
        }
    }

    fn path() -> PathBuf {
        let mut config_dir: PathBuf = dirs::config_dir().unwrap();
        config_dir.push("hue_mie");
        config_dir.push("config");
        config_dir.set_extension("toml");
        config_dir
    }

    pub fn from_file() -> Result<Config, Box<dyn std::error::Error>> {
        Config::parse(Config::path().to_str().unwrap())
    }

    pub fn write_file_to(self: &Config, path: &str) -> Result<(), Box<dyn std::error::Error>> {
        let str = toml::to_string(self)?;
        std::fs::write(path, str)?;
        Ok(())
    }

    pub fn write_file(self: &Config) -> Result<(), Box<dyn std::error::Error>> {
        self.write_file_to(Config::path().to_str().unwrap())
    }

    pub fn parse(path: &str) -> Result<Config, Box<dyn std::error::Error>> {
        println!("Reading path {:?}", path);
        let str = File::open(&path)
            .and_then(|mut file| {
                let mut config_toml = String::new();
                file.read_to_string(&mut config_toml)?;
                Ok(config_toml)
            })
            .unwrap_or_else(|_| String::from(""));
        let parsed = toml::from_str(&str)?;
        Ok(parsed)
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct HueConfig {
    #[serde(default = "HueConfig::default_bridge_ip")]
    pub bridge_ip: String,

    #[serde(default = "HueConfig::default_bridge_password")]
    pub bridge_password: String,
}

impl HueConfig {
    fn default_bridge_ip() -> String {
        String::from("192.168.178.50")
    }
    fn default_bridge_password() -> String {
        String::from("a-zKQed-fmtva4-gc0VJuVGrqaBf8t7xMEuJzUH2")
    }
}

impl Default for HueConfig {
    fn default() -> Self {
        HueConfig {
            bridge_ip: String::from("192.168.178.50"),
            bridge_password: String::from("a-zKQed-fmtva4-gc0VJuVGrqaBf8t7xMEuJzUH2"),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Transitions {
    #[serde(default = "Transitions::default_day_brightness")]
    pub day_brightness: f64,

    #[serde(default = "Transitions::default_day_temperature")]
    pub day_temperature: f64,

    #[serde(default = "Transitions::default_night_temperature")]
    pub night_temperature: f64,

    #[serde(default = "Transitions::default_night_brightness")]
    pub night_brightness: f64,

    #[serde(default = "Transitions::default_deep_night_brightness")]
    pub deep_night_brightness: f64,

    #[serde(default = "Transitions::default_deep_night_start_hour")]
    pub deep_night_start_hour: u8,

    #[serde(default = "Transitions::default_deep_night_end_hour")]
    pub deep_night_end_hour: u8,

    #[serde(default = "Transitions::default_sun_altitude_dawn_point")]
    pub sun_altitude_dawn_point: f64,

    #[serde(default = "Transitions::default_transition_time")]
    pub transition_time: f64,

    #[serde(default = "Transitions::default_brightness_cycle_length")]
    pub brightness_cycle_length: f64,

    #[serde(default = "Transitions::default_temperature_cycle_length")]
    pub temperature_cycle_length: f64,

    #[serde(default = "Transitions::default_brightness_cycle_amplitude")]
    pub brightness_cycle_amplitude: f64,

    #[serde(default = "Transitions::default_temperature_cycle_amplitude")]
    pub temperature_cycle_amplitude: f64,
}

impl Transitions {
    pub fn default_day_brightness() -> f64 {
        1.0
    }
    pub fn default_day_temperature() -> f64 {
        5700.0
    }
    pub fn default_night_temperature() -> f64 {
        2400.0
    }
    pub fn default_night_brightness() -> f64 {
        0.7
    }
    pub fn default_deep_night_brightness() -> f64 {
        0.0
    }
    pub fn default_deep_night_start_hour() -> u8 {
        23
    }
    pub fn default_deep_night_end_hour() -> u8 {
        6
    }
    pub fn default_sun_altitude_dawn_point() -> f64 {
        -0.4
    }
    pub fn default_transition_time() -> f64 {
        1.0
    }
    pub fn default_brightness_cycle_length() -> f64 {
        600_f64
    }
    pub fn default_temperature_cycle_length() -> f64 {
        700_f64
    }
    pub fn default_brightness_cycle_amplitude() -> f64 {
        30.0
    }
    pub fn default_temperature_cycle_amplitude() -> f64 {
        50.0
    }
}

impl Default for Transitions {
    fn default() -> Self {
        Transitions {
            day_brightness: 1.0,
            day_temperature: 5700.0,
            night_temperature: 2400.0,
            night_brightness: 0.7,
            deep_night_brightness: 0.0,
            deep_night_start_hour: 23,
            deep_night_end_hour: 6,
            sun_altitude_dawn_point: -0.4,
            transition_time: 1.0,
            brightness_cycle_length: 600_f64,
            temperature_cycle_length: 700_f64,
            brightness_cycle_amplitude: 30.0,
            temperature_cycle_amplitude: 50.0,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Location {
    #[serde(default = "Location::default_long")]
    pub long: f64,

    #[serde(default = "Location::default_lat")]
    pub lat: f64,
}

impl Location {
    pub fn as_geograph_point(self: &Location) -> astro::coords::GeographPoint {
        astro::coords::GeographPoint {
            long: self.long.to_radians(),
            lat: self.lat.to_radians(),
        }
    }

    pub fn default_long() -> f64 {
        5.387_826_6_f64
    }
    pub fn default_lat() -> f64 {
        52.156_111_3_f64
    }
}

impl Default for Location {
    fn default() -> Self {
        Location {
            long: 5.387_826_6_f64,
            lat: 52.156_111_3_f64,
        }
    }
}
