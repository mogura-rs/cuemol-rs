use crate::{CueMolError, Result};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LScrTime {
    millis: f64,
}

impl Default for LScrTime {
    fn default() -> Self {
        Self::new()
    }
}

impl LScrTime {
    pub fn new() -> Self {
        Self { millis: 0.0 }
    }

    pub fn milli_sec_real(&self) -> f64 {
        self.millis
    }

    pub fn set_milli_sec_real(&mut self, value: f64) {
        self.millis = value;
    }

    pub fn sec_real(&self) -> f64 {
        self.millis / 1000.0
    }

    pub fn set_sec_real(&mut self, value: f64) {
        self.millis = value * 1000.0;
    }

    pub fn script_string(&self) -> String {
        if self.millis.abs() < 1.0e-9 {
            return "0".to_string();
        }
        let total_millis = self.millis.round() as i64;
        let total_seconds = total_millis / 1000;
        let millis = total_millis.rem_euclid(1000);
        let hours = total_seconds / 3600;
        let minutes_total = total_seconds / 60;
        let minutes = (total_seconds % 3600) / 60;
        let seconds = total_seconds % 60;
        let sec_text = if millis == 0 {
            seconds.to_string()
        } else {
            let frac = format!("{millis:03}").trim_end_matches('0').to_string();
            format!("{seconds}.{frac}")
        };

        if hours > 0 {
            format!("{hours}:{minutes}:{sec_text}")
        } else if minutes_total > 0 {
            format!("{minutes_total}:{sec_text}")
        } else {
            sec_text
        }
    }

    pub fn strval(&self) -> String {
        self.script_string()
    }

    pub fn set_str_value(&mut self, value: &str) -> Result<()> {
        let parts = value.split(':').collect::<Vec<_>>();
        let seconds = match parts.as_slice() {
            [sec] => parse_time_part(sec)?,
            [min, sec] => parse_time_part(min)? * 60.0 + parse_time_part(sec)?,
            [hour, min, sec] => {
                parse_time_part(hour)? * 3600.0
                    + parse_time_part(min)? * 60.0
                    + parse_time_part(sec)?
            }
            _ => return Err(CueMolError::invalid_argument("invalid time format")),
        };
        self.set_sec_real(seconds);
        Ok(())
    }

    pub fn set_strval(&mut self, value: &str) -> Result<()> {
        self.set_str_value(value)
    }

    pub fn hour(&self) -> i64 {
        (self.millis.round() as i64 / 1000) / 3600
    }

    pub fn get_hour(&self) -> i64 {
        self.hour()
    }

    pub fn minute(&self, component: bool) -> i64 {
        let total = (self.millis.round() as i64 / 1000) / 60;
        if component {
            total % 60
        } else {
            total
        }
    }

    pub fn get_minute(&self, component: bool) -> i64 {
        self.minute(component)
    }

    pub fn second(&self, component: bool) -> i64 {
        let total = self.millis.round() as i64 / 1000;
        if component {
            total % 60
        } else {
            total
        }
    }

    pub fn get_second(&self, component: bool) -> i64 {
        self.second(component)
    }

    pub fn milli_sec(&self, component: bool) -> i64 {
        let total = self.millis.round() as i64;
        if component {
            total % 1000
        } else {
            total
        }
    }

    pub fn millisec(&self) -> i64 {
        self.milli_sec(false)
    }

    pub fn get_milli_sec(&self, component: bool) -> i64 {
        self.milli_sec(component)
    }

    pub fn equals(&self, rhs: &Self) -> bool {
        (self.millis - rhs.millis).abs() < 1.0e-6
    }
}

fn parse_time_part(value: &str) -> Result<f64> {
    if value.is_empty() || value.matches('.').count() > 1 {
        return Err(CueMolError::invalid_argument("invalid time format"));
    }
    value
        .parse::<f64>()
        .map_err(|_| CueMolError::invalid_argument("invalid time format"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lscrtime_formats_parses_and_extracts_reference_cases() {
        let mut time = LScrTime::new();
        assert_eq!(time.milli_sec_real(), 0.0);
        assert_eq!(time.sec_real(), 0.0);
        assert_eq!(time.script_string(), "0");

        time.set_milli_sec_real(5000.0);
        assert_eq!(time.sec_real(), 5.0);
        time.set_sec_real(123.456);
        assert!((time.milli_sec_real() - 123456.0).abs() < 1.0e-3);

        time.set_sec_real(123.0);
        assert_eq!(time.script_string(), "2:3");
        time.set_milli_sec_real(123.0);
        assert_eq!(time.script_string(), "0.123");
        time.set_sec_real(3661.0);
        assert_eq!(time.script_string(), "1:1:1");

        time.set_str_value("34:17:36.789").unwrap();
        assert!((time.milli_sec_real() - 123456789.0).abs() < 1.0);
        assert_eq!(time.hour(), 34);
        assert_eq!(time.minute(true), 17);
        assert_eq!(time.second(true), 36);
        assert_eq!(time.milli_sec(true), 789);
        assert_eq!(time.minute(false), 2057);
        assert_eq!(time.second(false), 123456);
        assert_eq!(time.milli_sec(false), 123456789);

        let mut other = LScrTime::new();
        other.set_sec_real(123456.789);
        assert!(time.equals(&other));
        assert!(LScrTime::new().set_str_value("1:2:3:4").is_err());
        assert!(LScrTime::new().set_str_value("1.2.3").is_err());
    }
}
