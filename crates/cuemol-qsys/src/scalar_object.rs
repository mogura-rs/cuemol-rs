use cuemol_core::Vector4;

#[derive(Debug, Clone, PartialEq)]
pub struct ScalarHistogram {
    pub min: f64,
    pub max: f64,
    pub bin_count: usize,
    pub maximum: f64,
    pub sigma: f64,
    pub values: Vec<f64>,
}

impl ScalarHistogram {
    pub fn to_json(&self) -> String {
        let values = self
            .values
            .iter()
            .map(|value| format!("{value:.6}"))
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "{{\"min\":{:.6},\"max\":{:.6},\"nbin\":{},\"nmax\":{:.6},\"sig\":{:.6},\"histo\":[{}]}}\n",
            self.min, self.max, self.bin_count, self.maximum, self.sigma, values
        )
    }
}

pub trait ScalarObject {
    fn col_no(&self) -> usize;
    fn row_no(&self) -> usize;
    fn sec_no(&self) -> usize;

    fn start_col(&self) -> isize;
    fn start_row(&self) -> isize;
    fn start_sec(&self) -> isize;

    fn col_grid_size(&self) -> f64;
    fn row_grid_size(&self) -> f64;
    fn sec_grid_size(&self) -> f64;

    fn min_density(&self) -> f64;
    fn max_density(&self) -> f64;
    fn mean_density(&self) -> f64;
    fn rmsd_density(&self) -> f64;

    fn level_base(&self) -> f64;
    fn level_step(&self) -> f64;

    fn center(&self) -> Vector4;
    fn origin(&self) -> Vector4;

    fn value_at(&self, position: Vector4) -> f64;
    fn is_in_range(&self, position: Vector4) -> bool;
    fn is_in_boundary(&self, col: isize, row: isize, sec: isize) -> bool;
    fn at_byte(&self, col: isize, row: isize, sec: isize) -> u8;
    fn at_float(&self, col: isize, row: isize, sec: isize) -> f64;
    fn conv_to_orth(&self, index: Vector4) -> Vector4;

    fn is_periodic_grid(&self) -> bool {
        false
    }

    fn den_sigma(&self) -> f64 {
        self.rmsd_density()
    }

    fn den_min(&self) -> f64 {
        self.min_density()
    }

    fn den_max(&self) -> f64 {
        self.max_density()
    }

    fn den_mean(&self) -> f64 {
        self.mean_density()
    }

    fn get_min_density(&self) -> f64 {
        self.min_density()
    }

    fn get_max_density(&self) -> f64 {
        self.max_density()
    }

    fn get_mean_density(&self) -> f64 {
        self.mean_density()
    }

    fn get_rmsd_density(&self) -> f64 {
        self.rmsd_density()
    }

    fn histogram(&self, min: f64, max: f64, bins: usize) -> ScalarHistogram {
        if bins == 0 || max <= min {
            return ScalarHistogram {
                min: self.min_density(),
                max: self.max_density(),
                bin_count: bins,
                maximum: 0.0,
                sigma: self.rmsd_density(),
                values: Vec::new(),
            };
        }

        let (hist_min, hist_max, base_width, base_histogram) = self.base_histogram();
        let target_width = (max - min) / bins as f64;
        let mut histogram = vec![0.0; bins];
        for (index, count) in base_histogram.into_iter().enumerate() {
            let x_low = hist_min + index as f64 * base_width;
            let x_high = x_low + base_width;
            let low_index = ((x_low - min) / target_width).floor() as isize;
            let high_index = ((x_high - min) / target_width).floor() as isize;
            let low_outside = low_index < 0 || low_index >= bins as isize;
            let high_outside = high_index < 0 || high_index >= bins as isize;
            if low_outside && high_outside {
                continue;
            }
            if low_index == high_index {
                histogram[low_index as usize] += count;
                continue;
            }

            let low_ratio = (((low_index + 1) as f64 * target_width + min) - x_low) / base_width;
            if !low_outside {
                histogram[low_index as usize] += count * low_ratio;
            }
            for target_index in (low_index + 1)..high_index {
                if (0..bins as isize).contains(&target_index) {
                    histogram[target_index as usize] += count * target_width / base_width;
                }
            }
            let high_ratio = (x_high - (high_index as f64 * target_width + min)) / base_width;
            if !high_outside {
                histogram[high_index as usize] += count * high_ratio;
            }
        }

        ScalarHistogram {
            min: hist_min,
            max: hist_max,
            bin_count: bins,
            maximum: histogram.iter().copied().fold(0.0_f64, f64::max),
            sigma: self.rmsd_density(),
            values: histogram,
        }
    }

    fn histogram_json(&self, min: f64, max: f64, bins: usize) -> String {
        self.histogram(min, max, bins).to_json()
    }

    fn base_histogram(&self) -> (f64, f64, f64, Vec<f64>) {
        let min = self.min_density();
        let max = self.max_density();
        let range = max - min;
        let requested_width = self.rmsd_density() / 1000.0;
        let count = if range > 0.0 && requested_width > 0.0 {
            (range / requested_width).floor().max(1.0) as usize
        } else {
            1
        };
        let width = if range > 0.0 {
            range / count as f64
        } else {
            1.0
        };
        let mut histogram = vec![0.0; count];
        for col in 0..self.col_no() {
            for row in 0..self.row_no() {
                for sec in 0..self.sec_no() {
                    let value = self.at_float(
                        self.start_col() + col as isize,
                        self.start_row() + row as isize,
                        self.start_sec() + sec as isize,
                    );
                    let index = ((value - min) / width).floor() as isize;
                    if (0..count as isize).contains(&index) {
                        histogram[index as usize] += 1.0;
                    }
                }
            }
        }
        (min, max, width, histogram)
    }

    fn get_histogram_json(&self, min: f64, max: f64, bins: usize) -> String {
        self.histogram_json(min, max, bins)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct UniformScalarObject;

    impl ScalarObject for UniformScalarObject {
        fn col_no(&self) -> usize {
            4
        }

        fn row_no(&self) -> usize {
            4
        }

        fn sec_no(&self) -> usize {
            4
        }

        fn start_col(&self) -> isize {
            0
        }

        fn start_row(&self) -> isize {
            0
        }

        fn start_sec(&self) -> isize {
            0
        }

        fn col_grid_size(&self) -> f64 {
            1.0
        }

        fn row_grid_size(&self) -> f64 {
            1.0
        }

        fn sec_grid_size(&self) -> f64 {
            1.0
        }

        fn min_density(&self) -> f64 {
            0.5
        }

        fn max_density(&self) -> f64 {
            1.5
        }

        fn mean_density(&self) -> f64 {
            1.0
        }

        fn rmsd_density(&self) -> f64 {
            0.1
        }

        fn level_base(&self) -> f64 {
            0.0
        }

        fn level_step(&self) -> f64 {
            0.1
        }

        fn center(&self) -> Vector4 {
            Vector4::new3(2.0, 2.0, 2.0)
        }

        fn origin(&self) -> Vector4 {
            Vector4::new3(0.0, 0.0, 0.0)
        }

        fn value_at(&self, _position: Vector4) -> f64 {
            1.0
        }

        fn is_in_range(&self, _position: Vector4) -> bool {
            true
        }

        fn is_in_boundary(&self, col: isize, row: isize, sec: isize) -> bool {
            (0..4).contains(&col) && (0..4).contains(&row) && (0..4).contains(&sec)
        }

        fn at_byte(&self, _col: isize, _row: isize, _sec: isize) -> u8 {
            128
        }

        fn at_float(&self, _col: isize, _row: isize, _sec: isize) -> f64 {
            1.0
        }

        fn conv_to_orth(&self, index: Vector4) -> Vector4 {
            index
        }
    }

    #[test]
    fn scalar_object_histogram_json_matches_reference_shape() {
        let object = UniformScalarObject;
        let json = object.histogram_json(0.5, 1.5, 10);
        assert!(json.contains("\"min\":0.500000"));
        assert!(json.contains("\"max\":1.500000"));
        assert!(json.contains("\"nbin\":10"));
        assert!(json.contains("\"nmax\":64.000000"));
        assert!(json.contains("\"sig\":0.100000"));
        assert!(!json.contains("mean"));
        let histogram = object.histogram(0.5, 1.5, 10);
        assert_eq!(histogram.bin_count, 10);
        assert_eq!(histogram.values.len(), 10);
        assert_eq!(histogram.maximum, 64.0);
        assert_eq!(object.den_sigma(), 0.1);
        assert_eq!(object.den_min(), 0.5);
        assert_eq!(object.den_max(), 1.5);
        assert_eq!(object.den_mean(), 1.0);

        let json5 = object.histogram_json(0.5, 1.5, 5);
        let json20 = object.histogram_json(0.5, 1.5, 20);
        assert_eq!(histogram_sum(&json5), 64.0);
        assert_eq!(histogram_sum(&json20), 64.0);
        assert_ne!(json5, json20);
    }

    fn histogram_sum(json: &str) -> f64 {
        json.split("\"histo\":[")
            .nth(1)
            .unwrap()
            .split(']')
            .next()
            .unwrap()
            .split(',')
            .map(|value| value.parse::<f64>().unwrap())
            .sum()
    }
}
