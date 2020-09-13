use chrono::prelude::*;
use log::{debug, error, info};
use philipshue::bridge::Bridge;
use philipshue::hue::LightStateChange;
use philipshue::hue::Scene;
use std::collections::BTreeMap;
use std::f64::consts::PI;
use std::ops::Add;
use std::time::Duration;
use std::time::SystemTime;
use std::{thread, time};

mod astro_calc;
mod config;

use config::Config;

use crate::config::Location;
use crate::config::Transitions;

extern crate env_logger;
#[macro_use]
extern crate serde_derive;

trait ExtraMath<T> {
    fn sigmoid(self) -> T;
}

impl ExtraMath<f64> for f64 {
    fn sigmoid(self) -> f64 {
        self.exp() / (self.exp() + 1_f64)
    }
}

impl ExtraMath<f32> for f32 {
    fn sigmoid(self) -> f32 {
        self.exp() / (self.exp() + 1_f32)
    }
}

fn kelvin_to_mired(kelvin: f64) -> f64 {
    1_000_000_f64 / kelvin
}

mod i16_extra {
    pub fn diff(left: u16, right: u16) -> u16 {
        if left > right {
            left - right
        } else {
            right - left
        }
    }

    pub fn is_close(left: u16, right: u16) -> bool {
        diff(left, right) < 40
    }
}

mod i8_extra {
    pub fn diff(left: u8, right: u8) -> u8 {
        if left > right {
            left - right
        } else {
            right - left
        }
    }

    pub fn is_close(left: u8, right: u8) -> bool {
        diff(left, right) < 6
    }
}

fn scene_is_active(bridge: &Bridge, scene: &Scene) -> bool {
    scene.lightstates.iter().fold(true, |b, (id, ls)| {
        if !b {
            false
        } else {
            let light = bridge.get_light(*id).unwrap();
            debug!("Light: {:?}", light);
            debug!("Scene: {:?}", ls);
            let tl = &(light.state);
            b && ls.bri.map_or(true, |b| i8_extra::is_close(b, tl.bri))
                && tl.ct.map_or(true, |c1| {
                    ls.ct.map_or(true, |c2| i16_extra::is_close(c1, c2))
                })
                && Some(tl.on) == ls.on
        }
    })
}

fn update_scene(bridge: &Bridge, id: &str, scene: &Scene, light_target: &LightTarget) {
    for (light, state) in scene.lightstates.iter() {
        match scene.lights.binary_search(&light) {
            Ok(idx) => {
                let mut ls: LightStateChange = state.clone();

                ls.transitiontime = Some(150);
                let rotation = ((idx as f64) / (scene.lights.len() as f64)) * PI * 2.;
                let this_light_target = light_target.clone().rotate(rotation);
                info!("Light target for {:?}: {:?}", light, this_light_target);
                ls.bri = Some(this_light_target.bri());
                ls.ct = Some(this_light_target.ct());
                ls.on = Some(this_light_target.on());
                info!("Light state for {:?} : {:?}", light, ls);
                match bridge.set_light_state_in_scene(&id, *light, &ls) {
                    Ok(_vec) => {
                        // Do nothing
                    }
                    Err(err) => error!("Could not set light state {:?} in scene id {:?}: {}", ls, id, err),
                }
            }
            Err(err) => error!("Could not find light {:?}: {}", light, err)
        }
    }
    //thread::sleep(time::Duration::from_millis(100));
}

#[derive(Clone, Debug)]
struct LightTarget {
    bri: f64,
    mired: f64,
    bri_phase: f64,
    mired_phase: f64,
    bri_amplitude: f64,
    mired_amplitude: f64,
}

impl LightTarget {
    fn target_color_temperature(transitions: &Transitions, sun_altitude: f64) -> f64 {
        (sun_altitude.to_degrees() / 3.).sigmoid()
            * (transitions.day_temperature - transitions.night_temperature)
            + transitions.night_temperature
    }

    fn target_brightness(transitions: &Transitions, sun_altitude: f64, hour: u8) -> f64 {
        if hour >= transitions.deep_night_start_hour || hour < transitions.deep_night_end_hour {
            transitions.deep_night_brightness
        } else {
            ((sun_altitude.to_degrees() - transitions.sun_altitude_dawn_point)
                / transitions.transition_time)
                .sigmoid()
                * (transitions.day_brightness - transitions.night_brightness)
                + transitions.night_brightness
        }
    }

    fn new(transitions: &Transitions, location: &Location) -> LightTarget {
        let sun_altitude = astro_calc::sun_altitude(Utc::now(), location.as_geograph_point());
        let now = Local::now();
        let seconds_from_midnight = now.num_seconds_from_midnight();

        debug!("Apparent altitude: {:5}", sun_altitude.to_degrees());
        LightTarget {
            bri: LightTarget::target_brightness(transitions, sun_altitude, now.hour() as u8),
            mired: kelvin_to_mired(LightTarget::target_color_temperature(
                transitions,
                sun_altitude,
            )),
            bri_phase: (f64::from(seconds_from_midnight) * 2.0 * PI
                / transitions.brightness_cycle_length) % (2.0 * PI),
            mired_phase: (f64::from(seconds_from_midnight) * 2.0 * PI
                / transitions.temperature_cycle_length) % (2.0 * PI),
            bri_amplitude: transitions.brightness_cycle_amplitude,
            mired_amplitude: transitions.temperature_cycle_amplitude,
        }
    }

    pub fn rotate(self: &LightTarget, angle: f64) -> LightTarget {
        let mut c = self.clone();
        c.bri_phase = (c.bri_phase + angle) % (PI * 2.);
        c.mired_phase = (c.mired_phase + angle) % (PI * 2.);
        c
    }

    pub fn ct(self: &LightTarget) -> u16 {
        (self.mired_phase.cos() * self.mired_amplitude + self.mired)
            .max(0.)
            .min(65535.) as u16
    }

    pub fn bri(self: &LightTarget) -> u8 {
        (self.bri_phase.cos() * self.bri_amplitude + self.bri * 255.)
            .max(0.)
            .min(255.) as u8
    }

    pub fn on(self: &LightTarget) -> bool {
        self.bri() != 0
    }
}

fn update_scenes(bridge: &Bridge, scenes: BTreeMap<String, Scene>, light_target: &LightTarget) {
    scenes
        .iter()
        .filter(|&(_, scene)| scene.name.to_lowercase().contains("dayshift"))
        .filter(|&(_, scene)| !scene.recycle)
        .for_each(|(scene_id, scene)| {
            debug!("Updating scene {}, scene_id: {}", scene.name, scene_id);
            match bridge.get_scene_with_states(&scene_id) {
                Ok(s) => {
                    let scene_active = scene_is_active(&bridge, &s);

                    update_scene(&bridge, &scene_id, &s, &light_target);

                    let sleep_duration = time::Duration::from_millis(150);
                    thread::sleep(sleep_duration);
                    info!(
                        "Scene {} is {}!",
                        scene.name,
                        if scene_active { "active" } else { "inactive" }
                    );
                    if scene_active {
                        bridge
                            .get_all_groups()
                            .unwrap()
                            .iter()
                            .filter(|&(_, group)| group.lights == scene.lights)
                            .filter(|&(_, group)| !group.recycle.unwrap_or(false))
                            .for_each(|(group_id, _)| {
                                debug!("Recall scene {} in group {}", scene_id, group_id);
                                bridge.recall_scene_in_group(*group_id, &scene_id);
                            })
                    }
                }
                Err(e) => {
                    error!("Could not find scene with id {:?}: {}", scene_id, e)
                }
            }
        });
}

fn setup_and_get_config() -> Result<Config, Box<dyn std::error::Error>> {
    let mut config = Config::from_file()?.clone();

    let hue_config = match config.hue {
        Some(hue_config) => hue_config,
        None => Config::get_hue_config()?,
    };
    config.hue = Some(hue_config.clone());
    info!("Config: {:?}", config);
    config.write_file()?;

    Ok(config)
}

fn create_bridge(config: &config::HueConfig) -> philipshue::bridge::Bridge {
    Bridge::new(config.bridge_ip.clone(), config.bridge_password.clone())
}

fn main() {
    env_logger::init();
    let config = match setup_and_get_config() {
        Ok(config) => config,
        Err(err) => {
            error!("Error while retrieving config: {:?}", err);
            std::process::exit(-1);
        }
    };

    let bridge: Bridge = create_bridge(&(config.hue.unwrap()));
    loop {
        let next_step = SystemTime::now().add(Duration::from_secs(15));
        let light_target = LightTarget::new(&(config.transitions), &(config.location));
        debug!("target: {:?}", light_target);

        match bridge.get_all_scenes() {
            Ok(scenes) => update_scenes(&bridge, scenes, &light_target),
            Err(err) => error!("Error: {}", err),
        }
        let sleep = next_step
            .duration_since(SystemTime::now())
            .unwrap_or(Duration::from_secs(0));
        thread::sleep(sleep);
    }
}
