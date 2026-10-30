pub const PALETTES: [(&str, [[u8; 3]; 5]); 5] = [
    (
        "藍と金",
        [
            [10, 18, 37],
            [27, 52, 76],
            [66, 112, 120],
            [197, 167, 101],
            [255, 232, 173],
        ],
    ),
    (
        "深海",
        [
            [7, 20, 39],
            [16, 65, 91],
            [25, 133, 150],
            [104, 207, 199],
            [222, 249, 223],
        ],
    ),
    (
        "夕映え",
        [
            [29, 17, 46],
            [90, 41, 93],
            [176, 75, 114],
            [238, 143, 126],
            [255, 224, 172],
        ],
    ),
    (
        "翡翠",
        [
            [12, 28, 30],
            [30, 72, 60],
            [85, 132, 91],
            [174, 193, 129],
            [242, 238, 191],
        ],
    ),
    (
        "月白",
        [
            [17, 22, 34],
            [50, 63, 83],
            [103, 125, 146],
            [179, 198, 209],
            [244, 242, 230],
        ],
    ),
];

pub fn color(palette: usize, value: f32) -> [u8; 3] {
    let stops = PALETTES[palette.min(PALETTES.len() - 1)].1;
    let value = if value.is_finite() {
        value.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let scaled = value * 4.0;
    let index = (scaled as usize).min(3);
    let t = scaled - index as f32;
    let t = t * t * (3.0 - 2.0 * t);
    std::array::from_fn(|channel| {
        (stops[index][channel] as f32 * (1.0 - t) + stops[index + 1][channel] as f32 * t).round()
            as u8
    })
}

pub fn rgb(cells: &[[f32; 2]], palette: usize, contrast: f32, inverted: bool) -> Vec<u8> {
    cells
        .iter()
        .flat_map(|cell| {
            let t = (cell[1] * 2.8).clamp(0.0, 1.0).powf(1.0 / contrast);
            color(palette, if inverted { 1.0 - t } else { t })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn palette_endpoints_and_nonfinite_values() {
        for (i, (_, stops)) in PALETTES.iter().enumerate() {
            assert_eq!(color(i, -1.0), stops[0]);
            assert_eq!(color(i, 1.0), stops[4]);
            assert_eq!(color(i, f32::NAN), stops[0]);
        }
    }
}
