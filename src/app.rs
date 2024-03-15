use arboard::Clipboard;
use std::fs::File;
use std::io::BufReader;

use skia::{AutoCanvasRestore, Color4f, IPoint, IRect, Rect, RoundOut, Surface, Vector};
use winit::event::{ElementState, Modifiers};
use winit::keyboard::{ModifiersState, NamedKey};
use winit::window::CursorIcon;

use crate::editor::{Editor, Movement, PaintOpts, TextPosition};

pub trait Layer {
    fn new() -> Self;
    fn get_active(&self, ctx: &mut Context<'_>) -> bool;
    fn set_active(&mut self, ctx: &mut Context<'_>, active: bool);
    fn on_backend_created(&mut self, _ctx: &mut Context<'_>) {}
    fn on_char(&mut self, _ctx: &mut Context<'_>, _c: char, _modifiers: Modifiers) -> bool {
        false
    }
    fn on_key(&mut self, _ctx: &mut Context<'_>, _key: NamedKey, _modifiers: Modifiers) -> bool {
        false
    }
    fn on_mouse(
        &mut self,
        _ctx: &mut Context<'_>,
        (_x, _y): (i32, i32),
        _input_state: ElementState,
        _modifiers: Modifiers,
    ) -> bool {
        false
    }
    // fn on_mouse_wheel(&mut self, delta: f32, x: i32, y: i32, modifier_key: skui::ModifierKey) -> bool;
    // fn on_touch(&mut self, owner: isize, input_state: skui::InputState, x: f32, y: f32) -> bool;
    // fn on_fling(&mut self, state: skui::InputState) -> bool;
    // fn on_pinch(&mut self, state: skui::InputState, scale: f32, (width, height): (u32, u32)) -> bool;
    // fn on_ui_state_changed(&mut self, state_name: SkString, state_value: SkString);
    fn on_pre_paint(&mut self, _ctx: &mut Context<'_>) {}
    fn on_paint(&mut self, _ctx: &mut Context<'_>, _surface: &mut Surface) {}
    fn on_resize(&mut self, _ctx: &mut Context<'_>, (_width, _height): (i32, i32)) {}
}

pub struct Context<'a> {
    pub window: &'a winit::window::Window,
    pub clipboard: Clipboard,
}

impl Context<'_> {
    pub fn invalidate(&mut self) {
        self.window.request_redraw();
    }

    pub fn get_clipboard_text(&mut self) -> Option<String> {
        self.clipboard.get_text().ok()
    }

    pub fn set_clipboard_text(&mut self, text: &str) {
        _ = self.clipboard.set_text(text);
    }

    pub fn set_cursor_icon(&self, icon: CursorIcon) {
        self.window.set_cursor_icon(icon)
    }
}

// todo: private
pub struct EditorLayer {
    pub path: String,
    pub editor: Editor,
    pub text_pos: TextPosition,
    pub mark_pos: Option<TextPosition>,
    /// window pixel position in file
    pub pos: i32,
    /// window width
    pub width: i32,
    /// window height
    pub height: i32,
    pub margin: i32,
    pub typeface_index: usize,
    pub font_size: f32,
    pub shift_down: bool,
    pub blink: bool,
    pub mouse_down: bool,
}

const FONT_SIZE: f32 = 18.;

impl Layer for EditorLayer {
    fn new() -> Self {
        Self {
            path: String::new(),
            editor: Editor::new(),
            text_pos: TextPosition::new(0, 0),
            mark_pos: None,
            pos: 0,
            width: 0,
            height: 0,
            margin: 10,
            typeface_index: 0,
            font_size: FONT_SIZE,
            shift_down: false,
            blink: false,
            mouse_down: false,
        }
    }

    fn get_active(&self, _ctx: &mut Context<'_>) -> bool {
        true
    }

    fn set_active(&mut self, _ctx: &mut Context<'_>, _active: bool) {}

    fn on_char(&mut self, ctx: &mut Context<'_>, c: char, modifiers: Modifiers) -> bool {
        let c = match c {
            '\r' => '\n',
            _ => c,
        };

        match modifiers.state() {
            state if state.is_empty() => {
                let mut buf = [0; 4];
                self.editor.insert(self.text_pos, c.encode_utf8(&mut buf));
                self.move_cursor(ctx, Movement::Right, false)
            }
            ModifiersState::CONTROL => match c {
                'p' => todo!(),
                's' => todo!(),
                'c' => todo!(),
                'x' => todo!(),
                'v' => match ctx.get_clipboard_text() {
                    Some(text) => {
                        self.text_pos = self.editor.insert(self.text_pos, &text);
                        ctx.invalidate();
                        true
                    }
                    None => false,
                },
                '0' => todo!(),
                '=' => todo!(),
                '+' => todo!(),
                '-' => todo!(),
                '_' => todo!(),
                _ => false,
            },
            _ => false,
        }
    }

    fn on_key(&mut self, ctx: &mut Context<'_>, key: NamedKey, modifiers: Modifiers) -> bool {
        use NamedKey::*;

        let shift = modifiers.state().shift_key();

        let mut delete = |mov| {
            let pos = match self.mark_pos {
                None => self.editor.mov(mov, self.text_pos),
                Some(mark_pos) => mark_pos,
            };
            let pos = self.editor.remove(TextPosition::range(pos, self.text_pos));
            self.mov(ctx, pos, shift);
            ctx.invalidate();
            true
        };

        match key {
            ArrowLeft => self.move_cursor(ctx, Movement::Left, shift),
            ArrowRight => self.move_cursor(ctx, Movement::Right, shift),
            ArrowUp => self.move_cursor(ctx, Movement::Up, shift),
            ArrowDown => self.move_cursor(ctx, Movement::Down, shift),
            Home => self.move_cursor(ctx, Movement::Home, shift),
            End => self.move_cursor(ctx, Movement::End, shift),
            Delete => delete(Movement::Right),
            Backspace => delete(Movement::Left),
            Enter => self.on_char(ctx, '\n', modifiers),
            _ => false,
        }
        // todo: Ctrl+left,right
    }

    fn on_mouse(
        &mut self,
        ctx: &mut Context<'_>,
        (x, y): (i32, i32),
        input_state: ElementState,
        modifiers: Modifiers,
    ) -> bool {
        match input_state {
            ElementState::Pressed => {
                self.mouse_down = true;
                match self
                    .editor
                    .get_position(IPoint::new(x - self.margin, y + self.pos - self.margin))
                {
                    //
                    Some(pos) => self.mov(ctx, pos, modifiers.state().shift_key()),
                    None => false,
                }
            }
            ElementState::Released => {
                self.mouse_down = false;
                false
            }
        }
    }

    fn on_paint(&mut self, _ctx: &mut Context<'_>, surface: &mut Surface) {
        let canvas = surface.canvas();
        let acr = AutoCanvasRestore::guard(canvas, true);
        canvas
            .clip_rect(
                Rect::from_xywh(0., 0., self.width as _, self.height as _),
                None,
                None,
            )
            .translate(Vector::new(self.margin as _, (self.margin - self.pos) as _));
        let alpha = if self.blink { 0. } else { 1. };
        let mut options = PaintOpts {
            cursor: Some(self.text_pos),
            cursor_color: Color4f::new(1., 0., 0., alpha),
            background_color: Color4f::new(0.8, 0.8, 0.8, 1.),
            ..Default::default()
        };
        if let Some(mark_pos) = self.mark_pos {
            options.selection = Some((mark_pos, self.text_pos));
        }
        self.editor.paint(canvas, options);
        drop(acr);
    }

    fn on_resize(&mut self, ctx: &mut Context<'_>, size @ (width, height): (i32, i32)) {
        if (self.width, self.height) != size {
            self.height = height;
            if self.width != width {
                self.width = width;
                self.editor.set_width(width - self.margin * 2);
            }
            ctx.invalidate();
        }
    }
}

impl EditorLayer {
    fn set_font(&mut self, _ctx: &mut Context<'_>) {
        todo!()
    }

    fn load_file(&mut self, _ctx: &mut Context<'_>, path: &str) {
        self.path = path.to_string();
        self.editor.load(BufReader::new(File::open(path).unwrap()));
    }

    fn scroll(delta: u32) -> bool {
        todo!()
    }

    fn move_cursor(&mut self, ctx: &mut Context<'_>, mov: Movement, shift: bool) -> bool {
        self.mov(ctx, self.editor.mov(mov, self.text_pos), shift)
    }

    fn mov(&mut self, ctx: &mut Context<'_>, pos: TextPosition, shift: bool) -> bool {
        if pos == self.text_pos {
            if !shift {
                self.mark_pos = None;
            }
            return false;
        }
        if shift != self.shift_down {
            self.mark_pos = shift.then_some(self.text_pos);
            self.shift_down = shift;
        }
        self.text_pos = pos;

        // scroll if needed.
        let cursor: IRect = self.editor.get_location(self.text_pos).unwrap().round_out();
        let temp = cursor.bottom - self.height + self.margin * 2;
        if self.pos < temp {
            self.pos = temp;
        } else if cursor.top < self.pos {
            self.pos = cursor.top;
        }
        ctx.invalidate();
        true
    }
}
