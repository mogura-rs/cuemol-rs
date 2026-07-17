//! Levenberg-Marquardt nonlinear least-squares minimizer.

use cuemol_core::{CueMolError, Result};

/// Residual evaluator used by [`LmMinimizer`].
pub trait LmEvaluator {
    /// Fill `residuals` for the parameter vector `parameters`.
    ///
    /// `iflag` is one for ordinary evaluations and two for finite-difference
    /// Jacobian probes, matching the callback convention of MINPACK `lmdif`.
    fn eval(&mut self, parameters: &[f64], residuals: &mut [f64], iflag: i32);
}

impl<F> LmEvaluator for F
where
    F: FnMut(&[f64], &mut [f64], i32),
{
    fn eval(&mut self, parameters: &[f64], residuals: &mut [f64], iflag: i32) {
        self(parameters, residuals, iflag);
    }
}

/// Result of a nonlinear least-squares minimization.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LmOutcome {
    pub info: i32,
    pub evaluations: usize,
    pub residual_norm: f64,
}

/// Levenberg-Marquardt nonlinear least-squares minimizer.
///
/// This is the Rust counterpart of CueMol2's `minpack::LMMinimizer`. The
/// Jacobian is estimated with forward differences and each damped normal
/// equation is solved with pivoted Gaussian elimination.
#[derive(Debug)]
pub struct LmMinimizer<E> {
    evaluator: E,
    residual_count: usize,
    parameter_count: usize,
    tolerance: f64,
    max_evaluations: usize,
    info: i32,
    residual_norm: f64,
    evaluations: usize,
}

impl<E: LmEvaluator> LmMinimizer<E> {
    pub fn new(
        evaluator: E,
        residual_count: usize,
        parameter_count: usize,
        tolerance: f64,
    ) -> Result<Self> {
        if residual_count == 0 || parameter_count == 0 {
            return Err(CueMolError::invalid_argument(
                "LM dimensions must be positive",
            ));
        }
        if residual_count < parameter_count {
            return Err(CueMolError::invalid_argument(
                "LM residual count must be at least the parameter count",
            ));
        }
        if !tolerance.is_finite() || tolerance <= 0.0 {
            return Err(CueMolError::invalid_argument(
                "LM tolerance must be positive",
            ));
        }
        Ok(Self {
            evaluator,
            residual_count,
            parameter_count,
            tolerance,
            max_evaluations: 200 * (parameter_count + 1),
            info: 0,
            residual_norm: f64::NAN,
            evaluations: 0,
        })
    }

    pub fn residual_count(&self) -> usize {
        self.residual_count
    }

    pub fn parameter_count(&self) -> usize {
        self.parameter_count
    }

    pub fn tolerance(&self) -> f64 {
        self.tolerance
    }

    pub fn set_tolerance(&mut self, tolerance: f64) -> Result<()> {
        if !tolerance.is_finite() || tolerance <= 0.0 {
            return Err(CueMolError::invalid_argument(
                "LM tolerance must be positive",
            ));
        }
        self.tolerance = tolerance;
        Ok(())
    }

    pub fn max_evaluations(&self) -> usize {
        self.max_evaluations
    }

    pub fn set_max_evaluations(&mut self, count: usize) -> Result<()> {
        if count == 0 {
            return Err(CueMolError::invalid_argument(
                "LM maximum evaluation count must be positive",
            ));
        }
        self.max_evaluations = count;
        Ok(())
    }

    pub fn info(&self) -> i32 {
        self.info
    }

    pub fn residual_norm(&self) -> f64 {
        self.residual_norm
    }

    pub fn chi_sq(&self) -> f64 {
        self.residual_norm()
    }

    pub fn evaluations(&self) -> usize {
        self.evaluations
    }

    pub fn evaluator(&self) -> &E {
        &self.evaluator
    }

    pub fn evaluator_mut(&mut self) -> &mut E {
        &mut self.evaluator
    }

    /// Minimize the residual Euclidean norm in place.
    pub fn minimize(&mut self, parameters: &mut [f64], residuals: &mut [f64]) -> Result<LmOutcome> {
        if parameters.len() != self.parameter_count {
            return Err(CueMolError::invalid_argument(format!(
                "LM expected {} parameters, got {}",
                self.parameter_count,
                parameters.len()
            )));
        }
        if residuals.len() != self.residual_count {
            return Err(CueMolError::invalid_argument(format!(
                "LM expected {} residuals, got {}",
                self.residual_count,
                residuals.len()
            )));
        }
        if parameters.iter().any(|value| !value.is_finite()) {
            return Err(CueMolError::invalid_argument(
                "LM parameters must be finite",
            ));
        }

        self.info = 0;
        self.evaluations = 0;
        self.evaluate(parameters, residuals, 1)?;
        let mut cost = sum_squares(residuals);
        let mut damping = 1.0e-3;
        let mut jacobian = vec![0.0; self.residual_count * self.parameter_count];
        let mut probe_residuals = vec![0.0; self.residual_count];
        let mut candidate_residuals = vec![0.0; self.residual_count];
        let mut candidate = parameters.to_vec();

        while self.evaluations < self.max_evaluations {
            self.finite_difference_jacobian(
                parameters,
                residuals,
                &mut probe_residuals,
                &mut jacobian,
            )?;

            let (normal, gradient) = normal_equations(
                &jacobian,
                residuals,
                self.residual_count,
                self.parameter_count,
            );
            let gradient_max = gradient
                .iter()
                .fold(0.0_f64, |max, value| max.max(value.abs()));
            if gradient_max <= self.tolerance {
                self.info = 4;
                break;
            }

            let mut accepted = false;
            let mut accepted_step_norm = 0.0;
            let mut accepted_reduction = 0.0;
            for _ in 0..16 {
                let mut damped = normal.clone();
                for index in 0..self.parameter_count {
                    let diagonal = normal[index * self.parameter_count + index].abs().max(1.0);
                    damped[index * self.parameter_count + index] += damping * diagonal;
                }
                let rhs = gradient.iter().map(|value| -*value).collect::<Vec<_>>();
                let Some(step) = solve_linear_system(damped, rhs, self.parameter_count) else {
                    damping *= 10.0;
                    continue;
                };
                accepted_step_norm = euclidean_norm(&step);
                for index in 0..self.parameter_count {
                    candidate[index] = parameters[index] + step[index];
                }
                self.evaluate(&candidate, &mut candidate_residuals, 1)?;
                let candidate_cost = sum_squares(&candidate_residuals);
                if candidate_cost < cost {
                    accepted_reduction = cost - candidate_cost;
                    cost = candidate_cost;
                    parameters.copy_from_slice(&candidate);
                    residuals.copy_from_slice(&candidate_residuals);
                    damping = (damping * 0.1).max(1.0e-15);
                    accepted = true;
                    break;
                }
                damping = (damping * 10.0).min(1.0e30);
                if self.evaluations >= self.max_evaluations {
                    break;
                }
            }

            if !accepted {
                if self.evaluations >= self.max_evaluations {
                    self.info = 5;
                } else {
                    self.info = 6;
                }
                break;
            }

            let parameter_norm = euclidean_norm(parameters);
            let function_converged =
                accepted_reduction <= self.tolerance * self.tolerance * cost.max(1.0);
            let step_converged =
                accepted_step_norm <= self.tolerance * (parameter_norm + self.tolerance);
            self.info = match (function_converged, step_converged) {
                (true, true) => 3,
                (true, false) => 1,
                (false, true) => 2,
                (false, false) => 0,
            };
            if self.info != 0 {
                break;
            }
        }

        if self.info == 0 {
            self.info = 5;
        }
        self.residual_norm = cost.sqrt();
        Ok(LmOutcome {
            info: self.info,
            evaluations: self.evaluations,
            residual_norm: self.residual_norm,
        })
    }

    fn evaluate(&mut self, parameters: &[f64], residuals: &mut [f64], iflag: i32) -> Result<()> {
        if self.evaluations >= self.max_evaluations {
            return Err(CueMolError::invalid_argument(
                "LM maximum evaluation count exceeded",
            ));
        }
        self.evaluator.eval(parameters, residuals, iflag);
        self.evaluations += 1;
        if residuals.iter().any(|value| !value.is_finite()) {
            return Err(CueMolError::invalid_argument(
                "LM evaluator returned a non-finite residual",
            ));
        }
        Ok(())
    }

    fn finite_difference_jacobian(
        &mut self,
        parameters: &mut [f64],
        residuals: &[f64],
        probe_residuals: &mut [f64],
        jacobian: &mut [f64],
    ) -> Result<()> {
        let epsilon = f64::EPSILON.sqrt();
        for column in 0..self.parameter_count {
            if self.evaluations >= self.max_evaluations {
                return Ok(());
            }
            let original = parameters[column];
            let step = epsilon * original.abs().max(1.0);
            parameters[column] = original + step;
            let evaluation = self.evaluate(parameters, probe_residuals, 2);
            parameters[column] = original;
            evaluation?;
            for row in 0..self.residual_count {
                jacobian[row * self.parameter_count + column] =
                    (probe_residuals[row] - residuals[row]) / step;
            }
        }
        Ok(())
    }

    pub fn calc_enorm(residuals: &[f64]) -> f64 {
        euclidean_norm(residuals)
    }
}

fn sum_squares(values: &[f64]) -> f64 {
    values.iter().map(|value| value * value).sum()
}

fn euclidean_norm(values: &[f64]) -> f64 {
    // Scaled accumulation avoids overflow and underflow, like MINPACK `enorm`.
    let mut scale = 0.0_f64;
    let mut sum = 1.0_f64;
    for value in values {
        let absolute = value.abs();
        if absolute == 0.0 {
            continue;
        }
        if scale < absolute {
            let ratio = scale / absolute;
            sum = 1.0 + sum * ratio * ratio;
            scale = absolute;
        } else {
            let ratio = absolute / scale;
            sum += ratio * ratio;
        }
    }
    if scale == 0.0 {
        0.0
    } else {
        scale * sum.sqrt()
    }
}

fn normal_equations(
    jacobian: &[f64],
    residuals: &[f64],
    residual_count: usize,
    parameter_count: usize,
) -> (Vec<f64>, Vec<f64>) {
    let mut normal = vec![0.0; parameter_count * parameter_count];
    let mut gradient = vec![0.0; parameter_count];
    for row in 0..residual_count {
        for column in 0..parameter_count {
            let value = jacobian[row * parameter_count + column];
            gradient[column] += value * residuals[row];
            for other in 0..=column {
                normal[column * parameter_count + other] +=
                    value * jacobian[row * parameter_count + other];
            }
        }
    }
    for row in 0..parameter_count {
        for column in 0..row {
            normal[column * parameter_count + row] = normal[row * parameter_count + column];
        }
    }
    (normal, gradient)
}

fn solve_linear_system(mut matrix: Vec<f64>, mut rhs: Vec<f64>, size: usize) -> Option<Vec<f64>> {
    for column in 0..size {
        let pivot = (column..size).max_by(|lhs, rhs_index| {
            matrix[*lhs * size + column]
                .abs()
                .total_cmp(&matrix[*rhs_index * size + column].abs())
        })?;
        let pivot_value = matrix[pivot * size + column];
        if !pivot_value.is_finite() || pivot_value.abs() <= f64::EPSILON {
            return None;
        }
        if pivot != column {
            for index in 0..size {
                matrix.swap(column * size + index, pivot * size + index);
            }
            rhs.swap(column, pivot);
        }
        for row in column + 1..size {
            let factor = matrix[row * size + column] / matrix[column * size + column];
            matrix[row * size + column] = 0.0;
            for index in column + 1..size {
                matrix[row * size + index] -= factor * matrix[column * size + index];
            }
            rhs[row] -= factor * rhs[column];
        }
    }

    let mut solution = vec![0.0; size];
    for row in (0..size).rev() {
        let mut value = rhs[row];
        for column in row + 1..size {
            value -= matrix[row * size + column] * solution[column];
        }
        solution[row] = value / matrix[row * size + row];
        if !solution[row].is_finite() {
            return None;
        }
    }
    Some(solution)
}

/// C++ compatibility name.
pub type LMMinimizer<E> = LmMinimizer<E>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lm_minimizer_solves_rosenbrock_residuals() {
        let evaluator = |x: &[f64], residuals: &mut [f64], _iflag: i32| {
            residuals[0] = 10.0 * (x[1] - x[0] * x[0]);
            residuals[1] = 1.0 - x[0];
        };
        let mut minimizer = LmMinimizer::new(evaluator, 2, 2, 1.0e-10).unwrap();
        let mut parameters = [-1.2, 1.0];
        let mut residuals = [0.0; 2];
        let outcome = minimizer.minimize(&mut parameters, &mut residuals).unwrap();
        assert!(outcome.info > 0);
        assert!((parameters[0] - 1.0).abs() < 1.0e-6);
        assert!((parameters[1] - 1.0).abs() < 1.0e-6);
        assert!(outcome.residual_norm < 1.0e-7);
        assert_eq!(outcome.residual_norm, minimizer.chi_sq());
    }

    #[test]
    fn lm_minimizer_fits_an_overdetermined_line() {
        let xs = [-2.0, -1.0, 0.0, 1.0, 2.0];
        let ys = [-3.0, -1.0, 1.0, 3.0, 5.0];
        let evaluator = move |parameters: &[f64], residuals: &mut [f64], _iflag: i32| {
            for (index, (&x, &y)) in xs.iter().zip(&ys).enumerate() {
                residuals[index] = parameters[0] * x + parameters[1] - y;
            }
        };
        let mut minimizer = LMMinimizer::new(evaluator, 5, 2, 1.0e-12).unwrap();
        let mut parameters = [0.0, 0.0];
        let mut residuals = [0.0; 5];
        minimizer.minimize(&mut parameters, &mut residuals).unwrap();
        assert!((parameters[0] - 2.0).abs() < 1.0e-9);
        assert!((parameters[1] - 1.0).abs() < 1.0e-9);
        assert!(LMMinimizer::<fn(&[f64], &mut [f64], i32)>::calc_enorm(&residuals) < 1.0e-9);
    }

    #[test]
    fn lm_minimizer_validates_dimensions_and_non_finite_residuals() {
        let evaluator = |_x: &[f64], residuals: &mut [f64], _iflag: i32| {
            residuals.fill(f64::NAN);
        };
        assert!(LmMinimizer::new(evaluator, 1, 2, 1.0e-5).is_err());

        let mut minimizer = LmMinimizer::new(evaluator, 2, 1, 1.0e-5).unwrap();
        assert!(minimizer.minimize(&mut [0.0], &mut [0.0; 2]).is_err());
    }
}
