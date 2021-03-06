use anyhow::{anyhow, Result};
use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::pixels::Color;
use std::time::Duration;

use ndarray::prelude::*;

const WIDTH: u32 = 800;
const HEIGHT: u32 = 600;

struct Texture<'s> {
    texture: sdl2::render::Texture<'s>,
    width: u32,
    height: u32,
    bytes_per_pixel: u32,
    backing_buf: Vec<u8>,
}

impl<'s> Texture<'s> {
    fn new(texture: sdl2::render::Texture<'s>) -> Texture<'s> {
        let query = texture.query();
        let width = query.width;
        let height = query.height;
        let bytes_per_pixel = query.format.byte_size_per_pixel() as u32;
        let backing_buf = vec![0u8; (width * height * bytes_per_pixel) as usize];
        Texture {
            texture,
            width,
            height,
            bytes_per_pixel,
            backing_buf,
        }
    }

    fn update(&mut self) {
        self.texture
            .update(
                None,
                &self.backing_buf[..],
                (self.bytes_per_pixel * self.width) as usize,
            )
            .expect("Texture update failed")
    }

    fn set_pixel(&mut self, col: u32, row: u32, color: sdl2::pixels::Color) {
        let offset = col * self.bytes_per_pixel + row * self.bytes_per_pixel * self.width;
        let offset = offset as usize;
        let query = self.texture.query();
        use std::convert::TryInto;
        let val = color.to_u32(&query.format.try_into().expect("Pixel conversion issue"));
        let bytes = val.to_ne_bytes();
        self.backing_buf[offset..offset + 4].copy_from_slice(&bytes[..]);
    }
}

pub fn main() -> Result<()> {
    // Init
    let sdl_context = sdl2::init().unwrap();
    let video_subsystem = sdl_context.video().unwrap();
    let window = video_subsystem
        .window("rust-sdl2 demo", WIDTH, HEIGHT)
        .position_centered()
        .build()
        .unwrap();
    let mut canvas = window.into_canvas().build().unwrap();

    let texture_creator = canvas.texture_creator();
    let screen = texture_creator.create_texture_streaming(None, WIDTH, HEIGHT)?;
    let mut screen = Texture::new(screen);

    // Setup density and local_sound_speed
    let mut density: Array2<f32> = Array::ones((WIDTH as usize, HEIGHT as usize));
    let mut local_sound_speed: Array2<f32> = Array::ones((WIDTH as usize, HEIGHT as usize));

    for x in 300..500 {
        local_sound_speed[(x, 350)] = 0.0;
        density[(x, 350)] = 100.0;
    }

    // Setup pressure
    let mut pressure: Array2<f32> = Array::zeros((WIDTH as usize, HEIGHT as usize));

    // Setup vx
    let mut vx: Array2<f32> = Array::zeros((WIDTH as usize, HEIGHT as usize));

    // Setup vy
    let mut vy: Array2<f32> = Array::zeros((WIDTH as usize, HEIGHT as usize));

    let mut event_pump = sdl_context.event_pump().unwrap();

    let del_spatial = 0.01; // 1cm?
    let del_t = 0.001; // 1ms?
    let c0 = 2.0; // m/s?
    let courant = c0 * del_t / del_spatial;

    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 10000,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create("sine.wav", spec).unwrap();
    let amplitude = i16::MAX as f32;

    let mut scale = 1.0;
    let mut speed = 0.25;
    let freq = 0.125f32;
    let (mic_x, mic_y) = (200, 90);

    let mut i = 0.0;
    'running: loop {
        i = i + 1.0;
        screen.set_pixel(mic_x as u32, mic_y as u32, Color::RGB(255, 255, 255));
        pressure[(400, 300)] = (i * freq).sin() * 5.0;
        if i as usize % 10 == 0 {
            println!("Val @ i of {} : {}", i, pressure[(mic_x, mic_y - 10)]);
        }
        writer
            .write_sample((pressure[(mic_x, mic_y - 10)] * amplitude) as i16)
            .unwrap();

        let mut max_pressure = 0.0f32;
        for m in 1..WIDTH - 1 {
            for n in 1..HEIGHT - 1 {
                let m = m as usize;
                let n = n as usize;

                let cvxp = 2.0 * courant / (density[(m + 1, n)] + density[(m, n)] * c0);
                let cvyp = 2.0 * courant / (density[(m, n)] + density[(m, n + 1)] * c0);
                let cvrp = density[(m, n)] * local_sound_speed[(m, n)].powi(2) * c0 * courant;
                // if i as usize % 10 == 0 && m * n % 1000 == 0 {
                //     dbg!(cvxp, cvyp, cvrp);
                // }

                vx[(m, n)] = vx[(m, n)] - cvxp * (pressure[(m + 1, n)] - pressure[(m, n)]);
                vy[(m, n)] = vy[(m, n)] - cvyp * (pressure[(m, n + 1)] - pressure[(m, n)]);
                pressure[(m, n)] = pressure[(m, n)]
                    - cvrp * ((vx[(m, n)] - vx[(m - 1, n)]) + (vy[(m, n)] - vy[(m, n - 1)]));

                // let color = ((pressure[(m, n)]).log10() + scale) as u8;
                let p = pressure[(m, n)];
                let color = if p > 0.0 {
                    Color::RGB((p * scale) as u8, 0, (density[(m, n)] * 2.0) as u8)
                } else {
                    Color::RGB(0, (-p * scale) as u8, (density[(m, n)] * 2.0) as u8)
                };

                screen.set_pixel(m as u32, n as u32, color);
                if pressure[(m, n)] > max_pressure {
                    max_pressure = pressure[(m, n)];
                }
            }
        }
        // screen.update();
        // println!(
        //     "Mean pressure: {}",
        //     pressure.mean().expect("Non empty array")
        // );
        if i as usize % 10 == 0 {
            println!("Max pressure: {}", max_pressure);
        }
        for i in 0..5 {
            for j in 0..5 {
                screen.set_pixel(
                    (mic_x + i) as u32,
                    (mic_y + j) as u32,
                    Color::RGB(255, 255, 255),
                );
            }
        }
        screen.update();
        canvas
            .copy(&screen.texture, None, None)
            .map_err(|e| anyhow!(e))?;
        for event in event_pump.poll_iter() {
            match event {
                Event::Quit { .. }
                | Event::KeyDown {
                    keycode: Some(Keycode::Escape),
                    ..
                } => break 'running,
                Event::KeyDown {
                    keycode: Some(Keycode::Equals),
                    ..
                } => {
                    scale *= 2.0;
                    println!("Scale is {}", scale);
                }
                Event::KeyDown {
                    keycode: Some(Keycode::Minus),
                    ..
                } => {
                    scale /= 2.0;
                    println!("Scale is {}", scale);
                }
                Event::KeyDown {
                    keycode: Some(Keycode::Num0),
                    ..
                } => {
                    speed *= 2.0;
                    println!("Speed is {}", speed);
                }
                Event::KeyDown {
                    keycode: Some(Keycode::Num9),
                    ..
                } => {
                    speed /= 2.0;
                    println!("Speed is {}", speed);
                }
                Event::MouseMotion {
                    mousestate: state,
                    x,
                    y,
                    ..
                } => {
                    if state.left() {
                        for i in 0..3 {
                            for j in 0..3 {
                                local_sound_speed[(x as usize + i, y as usize + j)] = 0.0;
                                density[(x as usize + i, y as usize + j)] = 100.0;
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        // The rest of the game loop goes here...

        canvas.present();
        // ::std::thread::sleep(Duration::new(0, 1_000_000_000u32 / 60));
    }
    Ok(())
}
