use eframe::glow::PROGRAM_INPUT;
use lazy_static::lazy_static;
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use std::f32::consts::PI;
use std::{collections::HashMap, sync::Mutex};
use tesseract::Tesseract;

use image::{DynamicImage, GenericImageView, Pixel, Rgb};

use crate::theme::Theme;

const PIXEL_REWARD_WIDTH: f32 = 968.0;
const PIXEL_REWARD_HEIGHT: f32 = 235.0;
const PIXEL_REWARD_YDISPLAY: f32 = 316.0;
const PIXEL_REWARD_LINE_HEIGHT: f32 = 48.0;

const PIXEL_INVENTORY_WIDTH: f32 = 1224.0;
const PIXEL_INVENTORY_HEIGHT: f32 = 770.0;
const PIXEL_INVENTORY_BLOCK: f32 = 169.0; // Inventory blocks are square
const PIXEL_INVENTORY_YDISPLAY: f32 = 199.0;
const PIXEL_INVENTORY_XDISPLAY: f32 = 76.0;
const PIXEL_INVENTORY_LINE_HEIGHT: f32 = 48.0;
const PIXEL_INVENTORY_VERTICAL_BLOCKSPACER: f32 = 42.0;
const PIXEL_INVENTORY_HORIZONTAL_BLOCKSPACER: f32 = 31.0;

pub fn detect_theme(image: &DynamicImage) -> Theme {
    let screen_scaling = if image.width() * 9 > image.height() * 16 {
        image.height() as f32 / 1080.0
    } else {
        image.width() as f32 / 1920.0
    };

    let line_height = PIXEL_REWARD_LINE_HEIGHT / 2.0 * screen_scaling;
    let most_width = PIXEL_REWARD_WIDTH * screen_scaling;

    let min_width = most_width / 4.0;

    let weights = (line_height as u32..image.height())
        .into_par_iter()
        .fold(HashMap::new, |mut weights: HashMap<Theme, f32>, y| {
            let perc = (y as f32 - line_height) / (image.height() as f32 - line_height);
            let total_width = min_width * perc + min_width;
            for x in 0..total_width as u32 {
                let closest = Theme::closest_from_color(
                    image
                        .get_pixel(x + (most_width - total_width) as u32 / 2, y)
                        .to_rgb(),
                );

                *weights.entry(closest.0).or_insert(0.0) += 1.0 / (1.0 + closest.1).powi(4)
            }
            weights
        })
        .reduce(HashMap::new, |mut a, b| {
            for (k, v) in b {
                *a.entry(k).or_insert(0.0) += v;
            }
            a
        });

    println!("{:#?}", weights);

    weights
        .iter()
        .max_by(|a, b| a.1.total_cmp(b.1))
        .unwrap()
        .0
        .to_owned()
}

pub fn extract_parts(image: &DynamicImage, theme: Theme) -> Vec<DynamicImage> {
    image.save("input.png").unwrap();
    let screen_scaling = if image.width() * 9 > image.height() * 16 {
        image.height() as f32 / 1080.0
    } else {
        image.width() as f32 / 1920.0
    };
    let line_height = (PIXEL_REWARD_LINE_HEIGHT / 2.0 * screen_scaling) as usize;

    let width = image.width() as f32;
    let height = image.height() as f32;
    let most_width = PIXEL_REWARD_WIDTH * screen_scaling;
    let most_left = width / 2.0 - most_width / 2.0;
    // Most Top = pixleRewardYDisplay - pixleRewardHeight + pixelRewardLineHeight
    //                   (316          -        235        +       44)    *    1.1    =    137
    let most_top = height / 2.0
        - ((PIXEL_REWARD_YDISPLAY - PIXEL_REWARD_HEIGHT + PIXEL_REWARD_LINE_HEIGHT)
            * screen_scaling);
    println!("most_top: {}", most_top);
    let most_bot =
        height / 2.0 - ((PIXEL_REWARD_YDISPLAY - PIXEL_REWARD_HEIGHT) * screen_scaling * 0.5);
    println!("most_bot: {}", most_bot);

    let prefilter = image.crop_imm(
        most_left as u32,
        most_top as u32,
        most_width as u32,
        (most_bot - most_top) as u32,
    );
    let mut prefilter_draw = prefilter.clone().into_rgb8();
    prefilter.save("prefilter.png").unwrap();

    let mut rows = Vec::<usize>::new();
    for y in 0..prefilter.height() {
        let mut count = 0;
        for x in 0..prefilter.width() {
            let color = prefilter.get_pixel(x, y).to_rgb();
            if theme.threshold_filter(color) {
                count += 1;
            }
        }
        rows.push(count);
    }

    let mut perc_weights = Vec::new();
    let mut top_weights = Vec::new();
    let mut mid_weights = Vec::new();
    let mut bot_weights = Vec::new();

    let top_line_100 = prefilter.height() as usize - line_height;
    let top_line_50 = line_height / 2;

    let mut scaling = -1.0;
    let mut lowest_weight = 0.0;
    for i in 0..50 {
        let y_from_top = prefilter.height() as usize
            - (i as f32 * (top_line_100 - top_line_50) as f32 / 50.0 + top_line_50 as f32) as usize;
        let scale = 50 + i;
        let scale_width = (prefilter.width() as f32 * scale as f32 / 100.0) as usize;

        let text_segments = [2.0, 4.0, 16.0, 21.0];
        let text_top = (screen_scaling * text_segments[0] * scale as f32 / 100.0) as usize;
        let text_top_bot = (screen_scaling * text_segments[1] * scale as f32 / 100.0) as usize;
        let text_both_bot = (screen_scaling * text_segments[2] * scale as f32 / 100.0) as usize;
        let text_tail_bot = (screen_scaling * text_segments[3] * scale as f32 / 100.0) as usize;

        // println!("");
        // println!("i: {}", i);
        // println!("y_from_top: {}", y_from_top);
        let mut w = 0.0;
        for loc in text_top..text_top_bot + 1 {
            w += (scale_width as f32 * 0.06 - rows[y_from_top + loc] as f32).abs();
            prefilter_draw.put_pixel(
                prefilter_draw.width() / 2 + i as u32,
                (y_from_top + loc) as u32,
                Rgb([255; 3]),
            );
        }
        top_weights.push(w);

        let mut w = 0.0;
        for loc in text_top_bot + 1..text_both_bot {
            if rows[y_from_top + loc] < scale_width / 15 {
                w += (scale_width as f32 * 0.26 - rows[y_from_top + loc] as f32) * 5.0;
            } else {
                w += (scale_width as f32 * 0.24 - rows[y_from_top + loc] as f32).abs();
            }
            prefilter_draw.put_pixel(
                prefilter_draw.width() / 2 + i as u32,
                (y_from_top + loc) as u32,
                Rgb([0, 255, 0]),
            );
        }
        mid_weights.push(w);

        let mut w = 0.0;
        for loc in text_both_bot..text_tail_bot {
            w += 10.0 * (scale_width as f32 * 0.007 - rows[y_from_top + loc] as f32).abs();
            prefilter_draw.put_pixel(
                prefilter_draw.width() / 2 + i as u32,
                (y_from_top + loc) as u32,
                Rgb([0, 0, 255]),
            );
        }
        bot_weights.push(w);

        top_weights[i] /= (text_top_bot - text_top + 1) as f32;
        mid_weights[i] /= (text_both_bot - text_top_bot - 2) as f32;
        bot_weights[i] /= (text_tail_bot - text_both_bot - 1) as f32;
        perc_weights.push(top_weights[i] + mid_weights[i] + bot_weights[i]);

        if scaling <= 0.0 || lowest_weight > perc_weights[i] {
            scaling = scale as f32;
            lowest_weight = perc_weights[i];
        }
    }

    println!("Scaling: {}", scaling);

    let mut top_five = [-1_isize; 5];
    for (i, _w) in perc_weights.iter().enumerate() {
        let mut slot: isize = 4;
        while slot != -1
            && top_five[slot as usize] != -1
            && perc_weights[i] > perc_weights[top_five[slot as usize] as usize]
        {
            slot -= 1;
        }

        if slot != -1 {
            for slot2 in 0..slot {
                top_five[slot2 as usize] = top_five[slot2 as usize + 1]
            }
            top_five[slot as usize] = i as isize;
        }
    }

    println!("top_five: {:?}", top_five);
    scaling = top_five[4] as f32 + 50.0;
    println!("scaling: {:?}", top_five);

    scaling /= 100.0;
    let high_scaling = if scaling < 1.0 {
        scaling + 0.01
    } else {
        scaling
    };
    let low_scaling = if scaling > 0.5 {
        scaling + 0.01
    } else {
        scaling
    };

    let crop_width = PIXEL_REWARD_WIDTH * screen_scaling * high_scaling;
    let crop_left = prefilter.width() as f32 / 2.0 - crop_width / 2.0;
    let crop_top = height / 2.0
        - (PIXEL_REWARD_YDISPLAY - PIXEL_REWARD_HEIGHT + PIXEL_REWARD_LINE_HEIGHT)
            * screen_scaling
            * high_scaling;
    //                  PIXEL_REWARD_YDISPLAY - PIXEL_REWARD_HEIGHT
    //                  316                   - 235                 = 81
    let crop_bot =
        height / 2.0 - (PIXEL_REWARD_YDISPLAY - PIXEL_REWARD_HEIGHT) * screen_scaling * low_scaling;
    let crop_hei = crop_bot - crop_top;
    let crop_top = crop_top - most_top;

    let partial_screenshot = DynamicImage::ImageRgb8(prefilter.into_rgb8()).crop_imm(
        crop_left as u32,
        crop_top as u32,
        crop_width as u32,
        crop_hei as u32,
    );

    // Draw top 5
    for (i, y) in top_five.iter().enumerate() {
        for x in 0..prefilter_draw.width() {
            prefilter_draw.put_pixel(x, *y as u32, Rgb([255 - i as u8 * 50, 0, 0]));
        }
    }
    // Draw histogram
    for (y, row) in rows.iter().enumerate() {
        for x in 0..*row {
            prefilter_draw.put_pixel(x as u32, y as u32, Rgb([0, 255, 0]));
        }
    }

    prefilter_draw.save("prefilter.png").unwrap();

    partial_screenshot.save("partial_screenshot.png").unwrap();

    filter_and_separate_parts_from_part_box(partial_screenshot, theme)
}

pub fn filter_and_separate_parts_from_part_box(
    image: DynamicImage,
    theme: Theme,
) -> Vec<DynamicImage> {
    let mut filtered = image.into_rgb8();

    let mut _weight = 0.0;
    let mut total_even = 0.0;
    let mut total_odd = 0.0;
    for x in 0..filtered.width() {
        let mut count = 0;
        for y in 0..filtered.height() {
            let pixel = filtered.get_pixel_mut(x, y);
            if theme.threshold_filter(*pixel) {
                *pixel = Rgb([0; 3]);
                count += 1;
            } else {
                *pixel = Rgb([255; 3]);
            }
        }

        count = count.min(filtered.height() / 3);
        let cosine = (8.0 * x as f32 * PI / filtered.width() as f32).cos();
        let cosine_thing = cosine.powi(3);

        // filtered.put_pixel(
        //     x,
        //     ((cosine_thing / 2.0 + 0.5) * (filtered.height() - 1) as f32) as u32,
        //     Rgb([255, 0, 0]),
        // );

        // println!("{}", cosine_thing);

        let this_weight = cosine_thing * count as f32;
        _weight += this_weight;

        if cosine < 0.0 {
            total_even -= this_weight;
        } else if cosine > 0.0 {
            total_odd += this_weight;
        }
    }

    filtered
        .save("filtered.png")
        .expect("Failed to write filtered image");

    if total_even == 0.0 && total_odd == 0.0 {
        return vec![];
    }

    let _total = total_even + total_odd;
    // println!("Even: {}", total_even / total);
    // println!("Odd: {}", total_odd / total);

    let box_width = filtered.width() / 4;
    let box_height = filtered.height();

    let mut curr_left = 0;
    let mut player_count = 4;

    if total_odd > total_even {
        curr_left = box_width / 2;
        player_count = 3;
    }

    let mut images = Vec::new();

    let dynamic_image = DynamicImage::ImageRgb8(filtered);
    for i in 0..player_count {
        let cropped = dynamic_image.crop_imm(curr_left + i * box_width, 0, box_width, box_height);
        cropped
            .save(format!("part-{}.png", i))
            .expect("Failed to write image");
        images.push(cropped);
    }

    images
}

pub fn normalize_string(string: &str) -> String {
    string.replace(|c: char| !c.is_ascii_alphabetic(), "")
}

pub fn image_to_string(tesseract: &mut Option<Tesseract>, image: &DynamicImage) -> String {
    let mut ocr = tesseract.take().unwrap();
    let buffer = image.as_flat_samples_u8().unwrap();
    ocr = ocr
        .set_frame(
            buffer.samples,
            image.width() as i32,
            image.height() as i32,
            3,
            3 * image.width() as i32,
        )
        .expect("Failed to set image");

    let result = ocr.get_text().expect("Failed to get text");
    tesseract.replace(ocr);

    result
}

lazy_static! {
    pub static ref OCR: Mutex<Option<Tesseract>> = Mutex::new(Some(
        Tesseract::new(None, Some("eng")).expect("Could not initialize Tesseract")
    ));
}

pub fn reward_image_to_reward_names(image: DynamicImage, theme: Option<Theme>) -> Vec<String> {
    let theme = theme.unwrap_or_else(|| detect_theme(&image));
    let parts = extract_parts(&image, theme);
    println!("Extracted part images");

    parts
        .iter()
        .map(|image| image_to_string(&mut OCR.lock().unwrap(), image))
        .collect()
}

pub fn inventory_image_to_inventory_names(
    image: DynamicImage,
    theme: Option<Theme>,
) -> Vec<String> {
    let theme = theme.unwrap_or_else(|| detect_theme(&image));
    let parts = extract_inventory(&image, &theme);

    let results: Vec<String> = parts
        .iter()
        .map(|image| image_to_string(&mut OCR.lock().unwrap(), image))
        .collect();
    println!("results: {:?}", results);
    results
}

// breaks on aspect ratios > 19:9
pub fn extract_inventory(image: &DynamicImage, theme: &Theme) -> Vec<DynamicImage> {
    let screen_scaling = if image.width() * 9 > image.height() * 16 {
        image.height() as f32 / 1080.0
    } else {
        image.width() as f32 / 1920.0
    };
    let mut final_images: Vec<DynamicImage> = Vec::new();
    image.save("input_inventory.png").unwrap();

    // crop four different parts hoizontaly starting at PIXEL_INVENTORY_YDISPLAY incorporate the vertical spacer
    for i in 0..4 {
        let crop_width = PIXEL_INVENTORY_BLOCK * screen_scaling * 6.0
            + PIXEL_INVENTORY_VERTICAL_BLOCKSPACER * 5.0;
        let partial_screenshot = image.crop_imm(
            PIXEL_INVENTORY_XDISPLAY as u32,
            PIXEL_INVENTORY_YDISPLAY as u32
                + i * (PIXEL_INVENTORY_BLOCK as u32
                    + PIXEL_INVENTORY_HORIZONTAL_BLOCKSPACER as u32),
            crop_width as u32,
            PIXEL_INVENTORY_BLOCK as u32,
        );

        partial_screenshot
            .save(format!("partial_screenshot_inventory_{}.png", i))
            .unwrap();
        let prefilter = draw_filter(&partial_screenshot, theme, screen_scaling);
        final_images.extend(filter_and_separate_inventory_blocks_from_inventory(
            partial_screenshot,
            theme,
            i,
        ));
    }
    final_images
}

pub fn filter_and_separate_inventory_blocks_from_inventory(
    image: DynamicImage,
    theme: &Theme,
    iteration: u32,
) -> Vec<DynamicImage> {
    let mut filtered = image.into_rgb8();

    let mut _weight = 0.0;
    let mut total_even = 0.0;
    let mut total_odd = 0.0;
    for x in 0..filtered.width() {
        let mut count = 0;
        for y in 0..filtered.height() {
            let pixel = filtered.get_pixel_mut(x, y);
            if theme.threshold_filter(*pixel) {
                *pixel = Rgb([0; 3]);
                count += 1;
            } else {
                *pixel = Rgb([255; 3]);
            }
        }

        count = count.min(filtered.height() / 3);
        let cosine = (8.0 * x as f32 * PI / filtered.width() as f32).cos();
        let cosine_thing = cosine.powi(3);

        let this_weight = cosine_thing * count as f32;
        _weight += this_weight;

        if cosine < 0.0 {
            total_even -= this_weight;
        } else if cosine > 0.0 {
            total_odd += this_weight;
        }
    }

    filtered
        .save("filtered.png")
        .expect("Failed to write filtered image");

    if total_even == 0.0 && total_odd == 0.0 {
        return vec![];
    }

    let _total = total_even + total_odd;
    // println!("Even: {}", total_even / total);
    // println!("Odd: {}", total_odd / total);

    let block_per_line_count = 6;

    let mut images = Vec::new();

    let dynamic_image = DynamicImage::ImageRgb8(filtered);
    for i in 0..block_per_line_count {
        //println!("Cropping block {}", i);
        let cropped = dynamic_image.crop_imm(
            i * (PIXEL_INVENTORY_BLOCK as u32 + PIXEL_INVENTORY_VERTICAL_BLOCKSPACER as u32),
            0,
            PIXEL_INVENTORY_BLOCK as u32,
            PIXEL_INVENTORY_BLOCK as u32,
        );
        cropped
            .save(format!(
                "partial_screenshot-{}_part-{}_inventory.png",
                iteration, i
            ))
            .expect("Failed to write image");
        images.push(cropped);
    }

    images
}

pub fn draw_filter(image: &DynamicImage, theme: &Theme, screen_scaling: f32) {
    let line_height = (PIXEL_REWARD_LINE_HEIGHT / 2.0 * screen_scaling) as usize;
    let mut image_draw = image.clone().to_rgb8();
    let mut rows = Vec::<usize>::new();
    for y in 0..image.height() {
        let mut count = 0;
        for x in 0..image.width() {
            let color = image.get_pixel(x, y).to_rgb();
            if theme.threshold_filter(color) {
                count += 1;
            }
        }
        rows.push(count);
    }

    let mut perc_weights = Vec::new();
    let mut top_weights = Vec::new();
    let mut mid_weights = Vec::new();
    let mut bot_weights = Vec::new();

    let top_line_100 = image.height() as usize - line_height;
    let top_line_50 = line_height / 2;

    let mut scaling = -1.0;
    let mut lowest_weight = 0.0;
    for i in 0..50 {
        let y_from_top = image.height() as usize
            - (i as f32 * (top_line_100 - top_line_50) as f32 / 50.0 + top_line_50 as f32) as usize;
        let scale = 50 + i;
        let scale_width = (image.width() as f32 * scale as f32 / 100.0) as usize;

        let text_segments = [2.0, 4.0, 16.0, 21.0];
        let text_top = (screen_scaling * text_segments[0] * scale as f32 / 100.0) as usize;
        let text_top_bot = (screen_scaling * text_segments[1] * scale as f32 / 100.0) as usize;
        let text_both_bot = (screen_scaling * text_segments[2] * scale as f32 / 100.0) as usize;
        let text_tail_bot = (screen_scaling * text_segments[3] * scale as f32 / 100.0) as usize;

        // println!("");
        // println!("i: {}", i);
        // println!("y_from_top: {}", y_from_top);
        let mut w = 0.0;
        for loc in text_top..text_top_bot + 1 {
            w += (scale_width as f32 * 0.06 - rows[y_from_top + loc] as f32).abs();
            image_draw.put_pixel(
                image_draw.width() / 2 + i as u32,
                (y_from_top + loc) as u32,
                Rgb([255; 3]),
            );
        }
        top_weights.push(w);

        let mut w = 0.0;
        for loc in text_top_bot + 1..text_both_bot {
            if rows[y_from_top + loc] < scale_width / 15 {
                w += (scale_width as f32 * 0.26 - rows[y_from_top + loc] as f32) * 5.0;
            } else {
                w += (scale_width as f32 * 0.24 - rows[y_from_top + loc] as f32).abs();
            }
            image_draw.put_pixel(
                image_draw.width() / 2 + i as u32,
                (y_from_top + loc) as u32,
                Rgb([0, 255, 0]),
            );
        }
        mid_weights.push(w);

        let mut w = 0.0;
        for loc in text_both_bot..text_tail_bot {
            w += 10.0 * (scale_width as f32 * 0.007 - rows[y_from_top + loc] as f32).abs();
            image_draw.put_pixel(
                image_draw.width() / 2 + i as u32,
                (y_from_top + loc) as u32,
                Rgb([0, 0, 255]),
            );
        }
        bot_weights.push(w);

        top_weights[i] /= (text_top_bot - text_top + 1) as f32;
        mid_weights[i] /= (text_both_bot - text_top_bot - 2) as f32;
        bot_weights[i] /= (text_tail_bot - text_both_bot - 1) as f32;
        perc_weights.push(top_weights[i] + mid_weights[i] + bot_weights[i]);

        if scaling <= 0.0 || lowest_weight > perc_weights[i] {
            scaling = scale as f32;
            lowest_weight = perc_weights[i];
        }
    }

    //println!("Scaling: {}", scaling);

    let mut top_five = [-1_isize; 5];
    for (i, _w) in perc_weights.iter().enumerate() {
        let mut slot: isize = 4;
        while slot != -1
            && top_five[slot as usize] != -1
            && perc_weights[i] > perc_weights[top_five[slot as usize] as usize]
        {
            slot -= 1;
        }

        if slot != -1 {
            for slot2 in 0..slot {
                top_five[slot2 as usize] = top_five[slot2 as usize + 1]
            }
            top_five[slot as usize] = i as isize;
        }
    }

    //println!("top_five: {:?}", top_five);
    scaling = top_five[4] as f32 + 50.0;
    //println!("scaling: {:?}", top_five);

    scaling /= 100.0;
    let high_scaling = if scaling < 1.0 {
        scaling + 0.01
    } else {
        scaling
    };
    let low_scaling = if scaling > 0.5 {
        scaling + 0.01
    } else {
        scaling
    };

    // Draw top 5
    for (i, y) in top_five.iter().enumerate() {
        for x in 0..image_draw.width() {
            image_draw.put_pixel(x, *y as u32, Rgb([255 - i as u8 * 50, 0, 0]));
        }
    }
    // Draw histogram
    for (y, row) in rows.iter().enumerate() {
        for x in 0..*row {
            image_draw.put_pixel(x as u32, y as u32, Rgb([0, 255, 0]));
        }
    }

    image_draw.save("prefilter_inventory.png").unwrap();
    image_draw;
}
