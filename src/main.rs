use std::ops::{Deref, DerefMut};

use arboard::Clipboard;
use skia::Rect;
use winit::dpi::PhysicalPosition;
use winit::window::CursorIcon;
use winit::{
    dpi::PhysicalSize,
    event::{ElementState, Event, Modifiers, WindowEvent},
    event_loop::EventLoop,
    keyboard::Key,
    window::WindowBuilder,
};

use crate::app::{Context, EditorLayer, Layer};

mod app;
mod editor;
mod shape;

fn main() -> Result<(), impl std::error::Error> {
    let event_loop = EventLoop::new().unwrap();

    let window = WindowBuilder::new()
        .with_title("skia-plain-text-editor")
        .build(&event_loop)
        .unwrap();

    let gc = softbuffer::Context::new(&window).unwrap();
    let mut surface = softbuffer::Surface::new(&gc, &window).unwrap();
    let PhysicalSize { width, height } = window.inner_size();
    surface
        .resize(width.try_into().unwrap(), height.try_into().unwrap())
        .unwrap();
    // todo: use skia::surfaces::wrap_pixels()
    // todo: resize
    let mut surface_sk = skia::surfaces::raster_n32_premul((width as i32, height as i32)).unwrap();

    let mut modifiers = Modifiers::default();
    let mut cursor_pos = PhysicalPosition::default();
    let ctx = &mut Context {
        window: &window,
        clipboard: Clipboard::new().unwrap(),
    };
    ctx.set_cursor_icon(CursorIcon::Text);
    let mut layer = EditorLayer::new();
    layer.on_resize(ctx, (width as _, height as _));

    event_loop.run(|event, elwt| {
        if let Event::WindowEvent { event, .. } = event {
            match event {
                WindowEvent::CloseRequested => elwt.exit(),
                WindowEvent::ModifiersChanged(m) => {
                    modifiers = m;
                }
                WindowEvent::CursorMoved { position, .. } => {
                    cursor_pos = position;
                }
                WindowEvent::MouseInput { state, .. } => {
                    let PhysicalPosition { x, y } = cursor_pos;
                    layer.on_mouse(ctx, (x as _, y as _), state, modifiers);
                }
                WindowEvent::KeyboardInput { event, .. } => 'keyboard: {
                    dbg!(&event.text);

                    if event.state == ElementState::Pressed {
                        if let Key::Named(key) = event.logical_key {
                            if layer.on_key(ctx, key, modifiers) {
                                break 'keyboard;
                            }
                        }
                        if let Some(text) = event.text {
                            if layer.on_char(ctx, text.deref().parse().unwrap(), modifiers) {
                                break 'keyboard;
                            }
                        }
                        if let Key::Character(c) = event.logical_key {
                            if layer.on_char(ctx, c.deref().parse().unwrap(), modifiers) {
                                break 'keyboard;
                            }
                        }
                    }
                }
                WindowEvent::Resized(PhysicalSize { width, height }) => {
                    layer.on_resize(ctx, (width as _, height as _));
                }
                WindowEvent::RedrawRequested => {
                    layer.on_paint(ctx, &mut surface_sk);

                    let image = surface_sk.image_snapshot();
                    let pixmap = image.peek_pixels().unwrap();
                    let mut buffer = surface.buffer_mut().unwrap();
                    buffer.deref_mut().copy_from_slice(pixmap.pixels().unwrap());
                    buffer.present().unwrap();
                }
                _ => {}
            }
        }
    })
}

const UNSET_RECT: Rect = Rect {
    left: f32::MIN,
    top: f32::MIN,
    right: f32::MIN,
    bottom: f32::MIN,
};
