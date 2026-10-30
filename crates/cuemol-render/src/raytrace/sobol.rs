use super::*;

fn owen(mut value: u32, seed: u32, base4: bool) -> u32 {
    value ^= value.wrapping_mul(0x3d20_adea);
    if base4 {
        value ^= (value >> 1) & (value << 1) & 0x5555_5555;
    }
    value = value.wrapping_add(seed).wrapping_mul((seed >> 16) | 1);
    if base4 {
        value ^= (value >> 1) & (value << 1) & 0x5555_5555;
    }
    value ^= value.wrapping_mul(0x0552_6c56);
    value ^ value.wrapping_mul(0x53a2_2864)
}

fn spread(mut value: u32) -> u32 {
    value &= 0xffff;
    for (shift, mask) in [
        (8, 0x00ff_00ff),
        (4, 0x0f0f_0f0f),
        (2, 0x3333_3333),
        (1, 0x5555_5555),
    ] {
        value = (value ^ (value << shift)) & mask;
    }
    value
}

pub(super) struct Sampler {
    first: u32,
    seed: u32,
}

impl Sampler {
    pub fn new(pixel: [u32; 2], count: usize) -> Self {
        let morton = (spread(pixel[0]) << 1) | spread(pixel[1]);
        let shuffled = owen(morton.reverse_bits(), 0, true).reverse_bits();
        Self {
            first: shuffled.wrapping_mul((count as u32).next_power_of_two()),
            seed: 0,
        }
    }

    pub fn glossy(pixel: u32) -> Self {
        fn hash(mut value: u32) -> u32 {
            value ^= value >> 16;
            value = value.wrapping_mul(0x7feb_352d);
            value ^= value >> 15;
            value = value.wrapping_mul(0x846c_a68b);
            value ^ (value >> 16)
        }
        Self {
            first: 0,
            seed: hash(pixel) ^ hash(0x474c_5359),
        }
    }

    pub fn sample(&self, sample: usize) -> [f32; 2] {
        let index = owen(
            self.first.wrapping_add(sample as u32).reverse_bits(),
            self.seed ^ 0xf8ad_e99a,
            false,
        );
        let mut direction = 1u32;
        let mut second = 0u32;
        for bit in 0..32 {
            if index & (1 << (31 - bit)) != 0 {
                second ^= direction;
            }
            direction ^= direction << 1;
        }
        let values = [index.reverse_bits(), second];
        let seeds = [self.seed ^ 0xe0aa_af76, self.seed ^ 0x9496_4d4e];
        std::array::from_fn(|i| {
            owen(values[i], seeds[i], false).reverse_bits() as f32 * (1. / 4_294_967_808.)
        })
    }

    pub fn direction(&self, normal: Vector4, sample: usize) -> Vector4 {
        let [angle, radial] = self.sample(sample);
        let (tangent, bitangent) = frame(normal);
        let radius = f64::from(radial).sqrt();
        let phi = std::f64::consts::TAU * f64::from(angle);
        normalize(
            tangent * (radius * phi.cos())
                + bitangent * (radius * phi.sin())
                + normal * (1. - f64::from(radial)).sqrt(),
        )
    }
}

pub(super) fn frame(normal: Vector4) -> (Vector4, Vector4) {
    let tangent = if normal.x.abs() < normal.y.abs() {
        Vector4::new3(0., normal.z, -normal.y)
    } else {
        Vector4::new3(-normal.z, 0., normal.x)
    }
    .normalize3d()
    .unwrap_or(Vector4::new3(1., 0., 0.));
    (tangent, cross(normal, tangent))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blue_noise_samples_match_the_reference_integrator() {
        // CueMol/umbreon 98f5438, pt2MakePixelSampler and pt2SobolBurley2D.
        let expected = [
            (0, 0, [0.228373274, 0.548622727]),
            (0, 7, [0.51815331, 0.0942359939]),
            (0, 31, [0.892102897, 0.920193911]),
            (1, 0, [0.206554428, 0.0689860657]),
            (1, 7, [0.0528185703, 0.955311358]),
            (1, 31, [0.0753768981, 0.737143338]),
            (19, 0, [0.302557975, 0.147061154]),
            (19, 7, [0.18267639, 0.369693637]),
            (19, 31, [0.582793474, 0.168086648]),
            (65535, 0, [0.750927866, 0.659687996]),
            (65535, 7, [0.569173217, 0.963864684]),
            (65535, 31, [0.59622848, 0.172443077]),
        ];
        for (x, sample, expected) in expected {
            assert_eq!(Sampler::new([x, x / 3], 32).sample(sample), expected);
        }
    }
}
