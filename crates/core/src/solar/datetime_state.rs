use chrono::{DateTime, TimeZone, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolarDateTimeState {
    pub year: i32,
    pub month: u32,
    pub day: u32,
    pub hour: u32,
    pub minute: u32,
    pub second: u32,
    pub timezone_offset_hours: f64,
    pub is_animating: bool,
    pub animation_speed: f32, // minutes per frame
}

impl Default for SolarDateTimeState {
    fn default() -> Self {
        Self {
            year: 2026,
            month: 6,
            day: 21, // Winter solstice in southern hemisphere / summer solstice in northern
            hour: 14,
            minute: 0,
            second: 0,
            timezone_offset_hours: 10.0, // Default Melbourne / Sydney (AEST)
            is_animating: false,
            animation_speed: 2.0, // 2 minutes per frame
        }
    }
}

impl SolarDateTimeState {
    pub fn new(year: i32, month: u32, day: u32, hour: u32, minute: u32, timezone_offset: f64) -> Self {
        Self {
            year,
            month,
            day,
            hour,
            minute,
            second: 0,
            timezone_offset_hours: timezone_offset,
            is_animating: false,
            animation_speed: 2.0,
        }
    }

    /// Get current DateTime in UTC
    pub fn to_utc_datetime(&self) -> DateTime<Utc> {
        // Calculate UTC hour by subtracting timezone offset
        let local_total_seconds = (self.hour as i64) * 3600 + (self.minute as i64) * 60 + (self.second as i64);
        let offset_seconds = (self.timezone_offset_hours * 3600.0) as i64;
        let mut utc_total_seconds = local_total_seconds - offset_seconds;

        let mut day_adjust = 0i64;
        while utc_total_seconds < 0 {
            utc_total_seconds += 86400;
            day_adjust -= 1;
        }
        while utc_total_seconds >= 86400 {
            utc_total_seconds -= 86400;
            day_adjust += 1;
        }

        let utc_hour = (utc_total_seconds / 3600) as u32;
        let utc_min = ((utc_total_seconds % 3600) / 60) as u32;
        let utc_sec = (utc_total_seconds % 60) as u32;

        let base_date = Utc.with_ymd_and_hms(self.year, self.month, self.day, utc_hour, utc_min, utc_sec);
        match base_date {
            chrono::LocalResult::Single(dt) => {
                if day_adjust != 0 {
                    dt + chrono::Duration::days(day_adjust)
                } else {
                    dt
                }
            }
            _ => Utc::now(),
        }
    }

    /// Get local time as fractional hours [0.0 .. 24.0]
    pub fn time_of_day_fractional(&self) -> f32 {
        self.hour as f32 + (self.minute as f32) / 60.0 + (self.second as f32) / 3600.0
    }

    /// Set local time from fractional hours [0.0 .. 24.0]
    pub fn set_time_of_day_fractional(&mut self, frac_hours: f32) {
        let clamped = (frac_hours % 24.0 + 24.0) % 24.0;
        let h = clamped.floor() as u32;
        let rem_min = (clamped - h as f32) * 60.0;
        let m = rem_min.floor() as u32;
        let s = ((rem_min - m as f32) * 60.0).round() as u32;

        self.hour = h;
        self.minute = m;
        self.second = s.min(59);
    }

    /// Advance time by delta seconds during playback animation
    pub fn update_animation(&mut self, dt_seconds: f32) {
        if !self.is_animating {
            return;
        }

        let added_minutes = self.animation_speed * dt_seconds * 30.0;
        let mut cur_time = self.time_of_day_fractional() + added_minutes / 60.0;
        if cur_time >= 24.0 {
            cur_time -= 24.0;
        }
        self.set_time_of_day_fractional(cur_time);
    }

    /// Quick preset jumpers
    pub fn set_summer_solstice_south(&mut self) {
        self.month = 12;
        self.day = 21;
    }

    pub fn set_winter_solstice_south(&mut self) {
        self.month = 6;
        self.day = 21;
    }

    pub fn set_spring_equinox(&mut self) {
        self.month = 9;
        self.day = 21;
    }

    pub fn set_autumn_equinox(&mut self) {
        self.month = 3;
        self.day = 21;
    }

    /// Set date and time to current real-world local time
    pub fn set_to_now(&mut self) {
        use chrono::{Datelike, Timelike, Local};
        let now = Local::now();
        self.year = now.year();
        self.month = now.month();
        self.day = now.day();
        self.hour = now.hour();
        self.minute = now.minute();
        self.second = now.second();
        let offset = now.offset().local_minus_utc() as f64 / 3600.0;
        self.timezone_offset_hours = offset;
    }

    /// Number of days in the currently selected month and year
    pub fn days_in_current_month(&self) -> u32 {
        match self.month {
            1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
            4 | 6 | 9 | 11 => 30,
            2 => {
                let y = self.year;
                if (y % 4 == 0 && y % 100 != 0) || (y % 400 == 0) {
                    29
                } else {
                    28
                }
            }
            _ => 30,
        }
    }
}

