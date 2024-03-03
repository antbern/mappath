use image::{DynamicImage, GenericImageView};

use crate::{Cell, GridMap};

pub fn parse_img(img: &DynamicImage) -> Result<GridMap<usize>, anyhow::Error> {
    let width = img.width() as usize;
    let height = img.height() as usize;

    let mut cells = vec![vec![Cell::Invalid; width as usize]; height as usize];

    for row in 0..height {
        for col in 0..width {
            let p = img.get_pixel(col as u32, row as u32);

            cells[row][col] = if p.0[0] < 128 {
                Cell::Invalid
            } else {
                Cell::Valid { cost: 1 }
            }
        }
    }

    Ok(GridMap {
        rows: height,
        columns: width,
        cells,
    })
}
