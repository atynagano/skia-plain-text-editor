use std::io::BufRead;
use std::ops::Range;

use skia::{
    font::Edging, Canvas, Color4f, Contains, Font, FontHinting, FontMgr, FontStyle, IPoint, Paint,
    Point, Rect, TextBlob,
};

use crate::{
    shape::{self, ShapeResult},
    UNSET_RECT,
};

pub struct Editor {
    lines: Vec<TextLine>,
    width: i32,
    height: i32,
    font: Font,
    font_mgr: FontMgr,
    needs_reshape: bool,
    locale: &'static str,
}

impl Editor {
    pub fn new() -> Self {
        // todo: font

        let font_mgr = FontMgr::new();
        let typeface = font_mgr
            .match_family_style("Arial", FontStyle::default())
            .unwrap();
        let mut font = Font::from_typeface(typeface.clone(), 17.0);
        font.set_edging(Edging::SubpixelAntiAlias);
        font.set_subpixel(true);
        font.set_hinting(FontHinting::Full);

        Editor {
            lines: Vec::new(),
            width: 0,
            height: 0,
            font,
            font_mgr,
            needs_reshape: false,
            locale: "en",
        }
    }

    pub fn get_height(&self) -> i32 {
        self.height
    }

    pub fn set_width(&mut self, w: i32) {
        if self.width != w {
            self.width = w;
            self.needs_reshape = true;
            for line in &mut self.lines {
                Self::mark_dirty(line);
            }
        }
    }

    pub fn font(&self) -> &Font {
        &self.font
    }

    pub fn set_font(&mut self, font: Font) {
        if self.font != font {
            self.font = font;
            self.needs_reshape = true;
            for line in &mut self.lines {
                Self::mark_dirty(line);
            }
        }
    }

    pub fn set_font_mgr(&mut self, font_mgr: FontMgr) {
        self.font_mgr = font_mgr;
        self.needs_reshape = true;
        for line in &mut self.lines {
            Self::mark_dirty(line);
        }
    }

    pub fn text(&self) -> &[TextLine] {
        &self.lines
    }

    pub fn line_height(&self, index: usize) -> i32 {
        self.lines[index].height
    }

    pub fn mov(&self, mov: Movement, mut pos: TextPosition) -> TextPosition {
        if self.lines.is_empty() {
            return TextPosition::new(0, 0);
        }
        if pos.paragraph_index >= self.lines.len() {
            pos.paragraph_index = self.lines.len() - 1;
            pos.text_byte_index = self.lines.last().unwrap().text.len();
        } else {
            let text = &*self.lines[pos.paragraph_index].text;
            pos.text_byte_index = (0..=pos.text_byte_index)
                .rev()
                .find(|&i| text.is_char_boundary(i))
                .unwrap();
        }

        assert!(pos.paragraph_index < self.lines.len());
        assert!(pos.text_byte_index <= self.lines[pos.paragraph_index].text.len());
        assert!(
            pos.text_byte_index == self.lines[pos.paragraph_index].text.len()
                || self.lines[pos.paragraph_index]
                    .text
                    .is_char_boundary(pos.text_byte_index)
        );

        match mov {
            Movement::Nowhere => {}
            Movement::Left => {
                if pos.text_byte_index == 0 {
                    if pos.paragraph_index > 0 {
                        pos.paragraph_index -= 1;
                        pos.text_byte_index = self.lines[pos.paragraph_index].text.len();
                    }
                } else {
                    let text = &*self.lines[pos.paragraph_index].text;
                    pos.text_byte_index = (0..pos.text_byte_index)
                        .rev()
                        .find(|&i| text.is_char_boundary(i))
                        .unwrap();
                }
            }
            Movement::Up => todo!(),
            Movement::Right => {
                let text = &*self.lines[pos.paragraph_index].text;
                if let Some(i) = (1..=4)
                    .map(|i| pos.text_byte_index + i)
                    .find(|&i| text.is_char_boundary(i))
                {
                    pos.text_byte_index = i;
                } else if pos.paragraph_index + 1 < self.lines.len() {
                    pos.paragraph_index += 1;
                    pos.text_byte_index = 0;
                }
            }
            Movement::Down => todo!(),
            Movement::Home => todo!(),
            Movement::End => todo!(),
            Movement::WordLeft => todo!(),
            Movement::WordRight => todo!(),
        }
        return pos;
    }

    pub fn get_position(&mut self, xy: IPoint) -> Option<TextPosition> {
        self.reshape_all();
        let mut approximate_position = None;
        for (j, line) in self.lines.iter().enumerate() {
            let mut line_rect = Rect::new(
                0.,
                line.origin.y as _,
                self.width as _,
                if let Some(l) = self.lines.get(j + 1) {
                    l.origin.y as _
                } else {
                    f32::MAX
                },
            );
            if let Some(blob) = &line.blob {
                line_rect.join(blob.bounds())
            }
            if !line_rect.contains(Point::from(xy)) {
                continue;
            }
            let pt = Point::from(xy - line.origin);
            for (i, pos) in line.cursor_pos.iter().enumerate() {
                if pos != &UNSET_RECT && pos.contains(pt) {
                    return Some(TextPosition::new(i, j));
                }
            }
            approximate_position = Some(TextPosition::new(
                if xy.x <= line.origin.x {
                    0
                } else {
                    line.text.len()
                },
                j,
            ));
        }
        approximate_position
    }

    pub fn get_location(&mut self, cursor: TextPosition) -> Option<Rect> {
        self.reshape_all();
        if self.lines.is_empty() {
            return None;
        }
        let cursor = self.mov(Movement::Nowhere, cursor);
        let line = &self.lines[cursor.paragraph_index];
        let mut pos = match line.cursor_pos.get(cursor.text_byte_index) {
            None => return None,
            Some(&pos) => pos,
        };
        pos.right = pos.left + 1.;
        pos.left -= 1.;
        Some(pos.with_offset(line.origin))
    }

    pub fn insert(&mut self, pos: TextPosition, text: &str) -> TextPosition {
        if text.is_empty() {
            return pos;
        }

        fn to_text_line(text: &str) -> TextLine {
            TextLine::new(text.into())
        }

        self.needs_reshape = true;
        let pos = self.mov(Movement::Nowhere, pos);

        let mut parts = text.split('\n');
        if self.lines.len() <= pos.paragraph_index {
            assert_eq!(pos.paragraph_index, self.lines.len());
            assert_eq!(pos.text_byte_index, 0);
            self.lines.extend(parts.map(to_text_line));
            return TextPosition::new(self.lines.last().unwrap().text.len(), self.lines.len());
        };

        Self::mark_dirty(&mut self.lines[pos.paragraph_index]);
        let first = parts.next().unwrap();
        if let Some(next) = parts.next() {
            let len = self.lines.len();
            let next_index = pos.paragraph_index + 1;
            self.lines.splice(
                next_index..next_index,
                [to_text_line(next)]
                    .into_iter()
                    .chain(parts.map(to_text_line)),
            );
            let last_index = pos.paragraph_index + self.lines.len() - len;
            let [head, .., foot] = &mut self.lines[pos.paragraph_index..=last_index] else {
                unreachable!()
            };
            let res = TextPosition::new(foot.text.len(), last_index);
            foot.text
                .push_str(head.text.drain(pos.text_byte_index..).as_str());
            head.text.push_str(first);
            res
        } else {
            // fast path
            self.lines[pos.paragraph_index]
                .text
                .insert_str(pos.text_byte_index, first);
            TextPosition::new(pos.text_byte_index + first.len(), pos.paragraph_index)
        }
    }

    pub fn remove(&mut self, range: Range<TextPosition>) -> TextPosition {
        let Range { start, end } = range;
        if start == end || start.paragraph_index >= self.lines.len() {
            return start;
        }
        self.needs_reshape = true;
        if let [head, .., foot] = &mut self.lines[start.paragraph_index..=end.paragraph_index] {
            Self::mark_dirty(head);
            head.text
                .replace_range(start.text_byte_index.., &foot.text[..=end.text_byte_index]);
            drop(
                self.lines
                    .drain(start.paragraph_index + 1..end.paragraph_index),
            );
        } else {
            let line = &mut self.lines[start.paragraph_index];
            Self::mark_dirty(line);
            line.text
                .replace_range(start.text_byte_index..end.text_byte_index, "");
        }
        start
    }

    pub fn copy(&self, range: Range<TextPosition>, dst: Option<&mut [u8]>) -> usize {
        todo!()
    }

    pub fn line_count(&self) -> usize {
        self.lines.len()
    }

    pub fn line(&self, i: usize) -> Option<StringView> {
        self.lines.get(i).map(|line| &*line.text)
    }

    pub fn paint(&mut self, canvas: &Canvas, options: PaintOpts) {
        self.reshape_all();

        canvas.draw_paint(&Paint::new(options.background_color, None));

        if self.lines.is_empty() {
            return;
        }

        if let Some((mark, current)) = options.selection {
            let selection = Paint::new(options.selection_color, None);
            let Range {
                start: mut pos,
                end,
            } = TextPosition::range(mark, current);
            while pos < end {
                let line = &self.lines[pos.paragraph_index];
                canvas.draw_rect(
                    line.cursor_pos[pos.text_byte_index].with_offset(line.origin),
                    &selection,
                );
                pos = self.mov(Movement::Right, pos);
            }
        }

        if let Some(cursor) = options.cursor {
            if let Some(rect) = self.get_location(cursor) {
                canvas.draw_rect(rect, &Paint::new(options.cursor_color, None));
            }
        }

        let foreground = Paint::new(options.foreground_color, None);
        for line in &self.lines {
            if let Some(blob) = &line.blob {
                canvas.draw_text_blob(blob, line.origin, &foreground);
            }
        }
    }

    fn mark_dirty(line: &mut TextLine) {
        line.blob = None;
        line.shaped = false;
        line.word_boundaries = vec![];
    }

    fn reshape_all(&mut self) {
        if !self.needs_reshape {
            return;
        }
        if self.lines.is_empty() {
            self.lines.push(TextLine::new(String::new()));
        }
        let shape_width = self.width as _;
        for line in self.lines.iter_mut().filter(|line| !line.shaped) {
            let ShapeResult {
                blob,
                line_break_offsets,
                glyph_bounds,
                word_breaks,
                vertical_advance,
            } = shape::shape(
                &line.text,
                &self.font,
                self.font_mgr.clone(),
                self.locale,
                shape_width,
            );
            line.blob = blob;
            line.cursor_pos = glyph_bounds;
            line.line_end_offsets = line_break_offsets;
            line.word_boundaries = word_breaks;
            line.height = vertical_advance;
            line.shaped = true;
        }
        self.height = self.lines.iter_mut().fold(0, |y, line| {
            line.origin = IPoint::new(0, y);
            y + line.height
        });
        self.needs_reshape = false;
    }

    pub fn load(&mut self, reader: impl BufRead) {
        self.lines
            .splice(.., reader.lines().map(|s| TextLine::new(s.unwrap())));
        self.needs_reshape = true;
    }
}

// note: paragraph first for PartialOrd macro
#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct TextPosition {
    /// logical line, based on hard newline characters.
    pub paragraph_index: usize,
    /// index into UTF-8 representation of line.
    pub text_byte_index: usize,
}

impl TextPosition {
    pub fn new(text_byte_index: usize, paragraph_index: usize) -> Self {
        Self {
            paragraph_index,
            text_byte_index,
        }
    }

    pub fn range(pos1: Self, pos2: Self) -> Range<Self> {
        let start = std::cmp::min(pos1, pos2);
        let end = std::cmp::max(pos1, pos2);
        Range { start, end }
    }
}

#[derive(Debug, Clone, Copy, Ord, PartialOrd, Eq, PartialEq)]
pub enum Movement {
    Nowhere,
    Left,
    Up,
    Right,
    Down,
    Home,
    End,
    WordLeft,
    WordRight,
}

pub struct PaintOpts {
    pub background_color: Color4f,
    pub foreground_color: Color4f,
    pub selection_color: Color4f,
    pub cursor_color: Color4f,
    pub selection: Option<(TextPosition, TextPosition)>,
    pub cursor: Option<TextPosition>,
}

impl Default for PaintOpts {
    fn default() -> Self {
        Self {
            background_color: Color4f::new(1.0, 1.0, 1.0, 1.0),
            foreground_color: Color4f::new(0.0, 0.0, 0.0, 1.0),
            selection_color: Color4f::new(0.729, 0.827, 0.988, 1.0),
            cursor_color: Color4f::new(1.0, 0.0, 0.0, 1.0),
            selection: Default::default(),
            cursor: Default::default(),
        }
    }
}

type StringSlice = String;
type StringView<'a> = &'a str;

struct TextLine {
    text: StringSlice,
    blob: Option<TextBlob>,
    cursor_pos: Vec<Rect>,
    line_end_offsets: Vec<usize>,
    word_boundaries: Vec<bool>,
    origin: IPoint,
    height: i32,
    shaped: bool,
}

impl TextLine {
    fn new(text: StringSlice) -> Self {
        Self {
            text,
            blob: Default::default(),
            cursor_pos: Default::default(),
            line_end_offsets: Default::default(),
            word_boundaries: Default::default(),
            origin: Default::default(),
            height: Default::default(),
            shaped: Default::default(),
        }
    }
}
