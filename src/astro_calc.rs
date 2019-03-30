use astro::time::julian_day;
use astro::time::CalType::Gregorian;
use astro::time::Date;
use astro::time::DayOfMonth;
use astro::time::*;
use astro::*;
use chrono::prelude::*;
use log::debug;

#[macro_export]
macro_rules! eq_frm_ecl2 {
    ($ecl_long: expr, $y: expr, $oblq_eclip: expr) => {{
        (
            coords::asc_frm_ecl($ecl_long, $y, $oblq_eclip),
            coords::dec_frm_ecl($ecl_long, $y, $oblq_eclip),
        )
    }};
}

pub fn decimal_day(day: &DayOfMonth) -> f64 {
    f64::from(day.day)
        + f64::from(day.hr) / 24.
        + f64::from(day.min) / (60. * 24.)
        + day.sec / (60.0 * 60. * 24.)
        - day.time_zone / 24.
}

pub fn sun_altitude(dt: DateTime<Utc>, geopoint: coords::GeographPoint) -> f64 {
    let day_of_month = DayOfMonth {
        day: dt.day() as u8,
        hr: dt.hour() as u8,
        min: dt.minute() as u8,
        sec: f64::from(dt.second()),
        time_zone: 0.0,
    };
    let date = Date {
        year: dt.year() as i16,
        month: dt.month() as u8,
        decimal_day: decimal_day(&day_of_month),
        cal_type: Gregorian,
    };

    let julian_day = julian_day(&date);
    debug!("julian_day: {}", julian_day);

    let (sun_ecl_point, _) = sun::geocent_ecl_pos(julian_day);
    debug!(
        "Ecliptic point of sun: {}, {}",
        sun_ecl_point.long, sun_ecl_point.lat
    );

    let oblq_eclip = ecliptic::mn_oblq_laskar(julian_day);
    let (asc, dec) = eq_frm_ecl2!(sun_ecl_point.long, sun_ecl_point.lat, oblq_eclip);
    debug!("Sun asc: {}, dec: {}", asc, dec);

    let hr_angle = mn_sidr(julian_day) + geopoint.long - asc;
    debug!("Hour angle: {}", hr_angle);

    let alt = coords::alt_frm_eq(hr_angle, dec, geopoint.lat);
    debug!("Real altitude: {}", alt);

    let apparent_alt = atmos::refrac_frm_true_alt(alt) + alt;
    debug!("Apparent altitude: {}", apparent_alt);

    apparent_alt
}
