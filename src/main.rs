// Hide the console window on Windows release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod audio;
mod fx;
mod game;
mod modes;
mod storage;
mod ui;

use macroquad::miniquad::conf::Icon;
use macroquad::prelude::*;

fn window_conf() -> Conf {
    Conf {
        window_title: "Rusty Scape".to_owned(),
        window_width: 960,
        window_height: 640,
        window_resizable: true,
        high_dpi: true,
        sample_count: 4,
        icon: Some(Icon {
            small: icon_pixels::<{ 16 * 16 * 4 }>(16),
            medium: icon_pixels::<{ 32 * 32 * 4 }>(32),
            big: icon_pixels::<{ 64 * 64 * 4 }>(64),
        }),
        ..Default::default()
    }
}

/// Draws the window icon procedurally: the rusty block hero with two eyes.
fn icon_pixels<const N: usize>(size: usize) -> [u8; N] {
    let mut px = [0u8; N];
    let f = size as f32;
    for y in 0..size {
        for x in 0..size {
            let (u, v) = (x as f32 / f, y as f32 / f);
            let i = (y * size + x) * 4;
            let body = (0.1..0.9).contains(&u) && (0.15..0.9).contains(&v);
            let eye =
                (0.55..0.7).contains(&v) && ((0.28..0.4).contains(&u) || (0.6..0.72).contains(&u));
            let rgba = if eye {
                [15, 15, 25, 255]
            } else if body && v < 0.25 {
                [250, 170, 90, 255]
            } else if body {
                [222, 115, 50, 255]
            } else {
                [0, 0, 0, 0]
            };
            px[i..i + 4].copy_from_slice(&rgba);
        }
    }
    px
}

#[macroquad::main(window_conf)]
async fn main() {
    rand::srand(macroquad::miniquad::date::now().to_bits());
    ui::load_font();

    // Sounds are synthesized at startup; show something friendly meanwhile.
    clear_background(Color::new(0.13, 0.12, 0.3, 1.0));
    ui::View::compute().apply(Vec2::ZERO);
    ui::text_shadow_centered("Loading...", ui::vw() / 2.0, ui::vh() / 2.0, 48.0, WHITE);
    set_default_camera();
    next_frame().await;

    let mut game = game::Game::new().await;
    loop {
        game.frame();
        next_frame().await;
    }
}
