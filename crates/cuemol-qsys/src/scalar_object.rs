use cuemol_core::Vector4;

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

    fn histogram_json(&self, min: f64, max: f64, bins: usize) -> String {
        let bins = bins.max(1);
        let mut histo = vec![0_usize; bins];
        let width = max - min;
        if width > 0.0 {
            for sec in 0..self.sec_no() {
                for row in 0..self.row_no() {
                    for col in 0..self.col_no() {
                        let value = self.at_float(
                            self.start_col() + col as isize,
                            self.start_row() + row as isize,
                            self.start_sec() + sec as isize,
                        );
                        if value < min || value > max {
                            continue;
                        }
                        let mut index = (((value - min) / width) * bins as f64).floor() as usize;
                        if index >= bins {
                            index = bins - 1;
                        }
                        histo[index] += 1;
                    }
                }
            }
        }

        let values = histo
            .iter()
            .map(usize::to_string)
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "{{\"min\":{},\"max\":{},\"mean\":{},\"rmsd\":{},\"histo\":[{}]}}",
            self.min_density(),
            self.max_density(),
            self.mean_density(),
            self.rmsd_density(),
            values
        )
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
        assert!(json.contains("histo"));
        assert!(json.contains("min"));
        assert!(json.contains("max"));

        let json5 = object.histogram_json(0.5, 1.5, 5);
        let json20 = object.histogram_json(0.5, 1.5, 20);
        assert!(json5.contains("histo"));
        assert!(json20.contains("histo"));
        assert_ne!(json5, json20);
    }
}
