use crate::UNSET_RECT;
use skia::shaper::run_handler::{Buffer, RunInfo};
use skia::{
    scalar, Font, FontMetrics, FontMgr, GlyphId, Point, Rect, Shaper, Size, TextBlob,
    TextBlobBuilder, Vector,
};
use std::ptr::NonNull;

struct TextBlobAlloc {
    builder: TextBlobBuilder,
    glyphs: Option<NonNull<[GlyphId]>>,
    positions: Option<NonNull<[Point]>>,
    // text: Option<NonNull<[u8]>>,
    clusters: Option<NonNull<[u32]>>,
}

impl Default for TextBlobAlloc {
    fn default() -> Self {
        Self {
            builder: TextBlobBuilder::new(),
            glyphs: None,
            positions: None,
            // text: None,
            clusters: None,
        }
    }
}

impl TextBlobAlloc {
    fn cache(&mut self) -> (&mut [GlyphId], &mut [Point], &mut [u32]) {
        unsafe {
            (
                self.glyphs.unwrap().as_mut(),
                self.positions.unwrap().as_mut(),
                // self.text.unwrap().as_mut(),
                self.clusters.unwrap().as_mut(),
            )
        }
    }

    fn alloc_run_text_pos(
        &mut self,
        font: &Font,
        count: usize,
        text_byte_count: usize,
        bounds: Option<&Rect>,
    ) -> (&mut [GlyphId], &mut [Point], &mut [u8], &mut [u32]) {
        let (glyphs, positions, text, clusters) =
            self.builder
                .alloc_run_text_pos(font, count, text_byte_count, bounds);
        self.glyphs = NonNull::new(glyphs);
        self.positions = NonNull::new(positions);
        // self.text = NonNull::new(text);
        self.clusters = NonNull::new(clusters);
        (glyphs, positions, text, clusters)
    }

    fn make(&mut self) -> Option<TextBlob> {
        self.builder.make()
    }
}

struct RunHandler<
    'a,
    C: FnMut(&str, &[GlyphId], &[Point], &[u32], &Font) = fn(
        &str,
        &[GlyphId],
        &[Point],
        &[u32],
        &Font,
    ),
> {
    builder: TextBlobAlloc,
    line_end_offsets: Vec<usize>,
    // current_glyphs: Vec<GlyphId>,
    // current_points: Vec<Point>,
    callback_function: Option<C>,
    text: &'a str,
    text_offset: usize,
    // clusters: &'a [u32],
    cluster_offset: u32,
    // glyph_count: usize,
    max_run_ascent: scalar,
    max_run_descent: scalar,
    max_run_leading: scalar,
    current_position: Point,
    offset: Point,
}

impl<C: FnMut(&str, &[GlyphId], &[Point], &[u32], &Font)> skia::shaper::RunHandler
    for RunHandler<'_, C>
{
    fn begin_line(&mut self) {
        self.current_position = self.offset;
        self.max_run_ascent = 0.;
        self.max_run_descent = 0.;
        self.max_run_leading = 0.;
    }

    fn run_info(&mut self, info: &RunInfo) {
        let (_, metrics) = info.font.metrics();
        self.max_run_ascent = self.max_run_ascent.min(metrics.ascent);
        self.max_run_descent = self.max_run_descent.max(metrics.descent);
        self.max_run_leading = self.max_run_leading.max(metrics.leading);
    }

    fn commit_run_info(&mut self) {
        self.current_position.y -= self.max_run_ascent;
    }

    fn run_buffer(&mut self, info: &RunInfo) -> Buffer {
        let (glyphs, positions, text, clusters) = self.builder.alloc_run_text_pos(
            info.font,
            info.glyph_count,
            info.utf8_range.len(),
            None,
        );
        assert_eq!(glyphs.len(), info.glyph_count);
        text.copy_from_slice(&self.text.as_bytes()[info.utf8_range.clone()]);

        Buffer {
            glyphs,
            positions,
            offsets: None,
            clusters: Some(clusters),
            point: self.current_position,
        }
    }

    fn commit_run_buffer(&mut self, info: &RunInfo) {
        let (glyphs, positions, clusters) = self.builder.cache();
        if let Some(callback) = &mut self.callback_function {
            callback(
                self.text.split_at(info.utf8_range.end).0,
                glyphs,
                positions,
                clusters,
                info.font,
            );
        }
        assert!(0 <= self.cluster_offset);
        for cluster in clusters {
            *cluster = cluster.checked_sub(self.cluster_offset).unwrap();
        }
        self.current_position += info.advance;
        self.text_offset = self.text_offset.max(info.utf8_range.end);
    }

    fn commit_line(&mut self) {
        if self.line_end_offsets.is_empty()
            || self.text_offset > *self.line_end_offsets.last().unwrap()
        {
            // Ensure that fLineEndOffsets is monotonic.
            self.line_end_offsets.push(self.text_offset);
        }
        self.offset += Point::new(
            0.,
            self.max_run_descent + self.max_run_leading - self.max_run_ascent,
        );
    }
}

impl<'a, C: FnMut(&str, &[GlyphId], &[Point], &[u32], &Font)> RunHandler<'a, C> {
    fn new(text: &'a str) -> Self {
        Self {
            builder: Default::default(),
            line_end_offsets: Default::default(),
            // current_glyphs: vec![],
            // current_points: vec![],
            callback_function: Default::default(),
            text,
            text_offset: Default::default(),
            // clusters: &[],
            cluster_offset: Default::default(),
            max_run_ascent: Default::default(),
            max_run_descent: Default::default(),
            max_run_leading: Default::default(),
            current_position: Default::default(),
            offset: Default::default(),
        }
    }

    fn set_run_callback(&mut self, callback: C) {
        self.callback_function = Some(callback);
    }

    fn make_blob(&mut self) -> Option<TextBlob> {
        self.builder.make()
    }

    fn final_rect(&self, font: &Font) -> Rect {
        if self.max_run_ascent == 0. || self.max_run_descent == 0. {
            let (_, metrics) = font.metrics();
            Rect::from_point_and_size(
                self.current_position,
                Size::new(font.size(), metrics.descent - metrics.ascent),
            )
        } else {
            Rect::new(
                self.current_position.x,
                self.current_position.y + self.max_run_ascent,
                self.current_position.x + font.size(),
                self.current_position.y + self.max_run_descent,
            )
        }
    }
}

fn selection_box(metrics: &FontMetrics, mut advance: f32, pos: Point) -> Rect {
    if advance.abs() < 1. {
        advance = f32::copysign(1., advance);
    }
    Rect::new(
        pos.x,
        pos.y + metrics.ascent,
        pos.x + advance,
        pos.y + metrics.descent,
    )
}

fn set_character_bounds(
    cursors: &mut [Rect],
    text: &str,
    glyphs: &[GlyphId],
    positions: &[Point],
    clusters: &[u32],
    font: &Font,
) {
    assert!(glyphs.len() > 0);
    assert_eq!(glyphs.len(), clusters.len());

    let (_, metrics) = font.metrics();
    let mut advances = vec![0.; glyphs.len()];
    font.get_widths(glyphs, &mut advances);

    // Loop over each cluster in this run.
    let mut cluster_start = 0;
    for (glyph_index, (&glyph, &text_begin)) in glyphs.iter().zip(clusters).enumerate() {
        if glyph_index + 1 < glyphs.len() && clusters[glyph_index] == clusters[glyph_index + 1] {
            // multi-glyph cluster
            continue;
        }
        let mut text_end = text.len() as u32;
        for &cluster in clusters {
            if cluster >= text_end {
                text_end = cluster + 1;
            }
        }
        for &cluster in clusters {
            if cluster > text_begin && cluster < text_end {
                text_end = cluster;
                if text_end == text_begin + 1 {
                    break;
                }
            }
        }
        // todo
        let text_begin = text_begin as usize;
        let text_end = text_end as usize;
        assert!(glyph_index + 1 > cluster_start);
        let cluster_glyph_positions = &positions[cluster_start..=glyph_index];
        let cluster_advances = &advances[cluster_start..=glyph_index];
        // for next loop
        cluster_start = glyph_index + 1;

        let mut zipped = cluster_advances.iter().zip(cluster_glyph_positions);
        let (&cluster_advance, &cluster_glyph_position) = zipped.next().unwrap();
        let mut cluster_box = selection_box(&metrics, cluster_advance, cluster_glyph_position);
        for (&cluster_advance, &cluster_glyph_position) in zipped {
            // multiple glyphs
            cluster_box.join(selection_box(
                &metrics,
                cluster_advance,
                cluster_glyph_position,
            ));
        }

        if text_begin + 1 == text_end {
            // single byte, fast path.
            cursors[text_begin] = cluster_box;
            continue;
        }
        let code_point = text.split_at(text_end).0.split_at(text_begin).1;
        let code_point_count = code_point.chars().count();
        if code_point_count == 1 {
            // single codepoint, fast path.
            cursors[text_begin] = cluster_box;
            continue;
        }

        let width = cluster_box.width() / code_point_count as f32;
        assert!(width > 0.);
        let base = Rect {
            right: cluster_box.left + width,
            ..cluster_box
        };
        let mut cursors = &mut cursors[text_begin..];
        for (i, (j, _)) in code_point.char_indices().enumerate() {
            cursors[j] = base.with_offset(Vector::new(width * i as f32, 0.));
        }
    }
}

pub struct ShapeResult {
    pub blob: Option<TextBlob>,
    pub line_break_offsets: Vec<usize>,
    pub glyph_bounds: Vec<Rect>,
    pub word_breaks: Vec<bool>,
    pub vertical_advance: i32,
}

pub fn shape(text: &str, font: &Font, font_mgr: FontMgr, locale: &str, width: f32) -> ShapeResult {
    let height = font.spacing();
    let vertical_advance = height.ceil() as _;

    let shaper = Shaper::new_shape_then_wrap(None).unwrap();
    let mut glyph_bounds = vec![UNSET_RECT; text.len()];
    // cursors.splice(.., std::iter::repeat(UNSET_RECT).take(text.len()));
    let mut handler = RunHandler::new(text);
    handler.set_run_callback(|text, glyphs, positions, clusters, font| {
        set_character_bounds(&mut glyph_bounds, text, glyphs, positions, clusters, font)
    });

    /*
    let handler = RunHandler::new(utf8_text);

    handler.set_run_callback();

    const BIDI_LEVEL_LTR: u8 = 0;
    // for bidirectional text (such as arabian)
    let mut bidi = Shaper::new_bidi_run_iterator(utf8_text, BIDI_LEVEL_LTR).unwrap();
    // for multiple languages
    let mut lang = Shaper::new_std_language_run_iterator(utf8_text).unwrap();
    // for multiple charsets
    let mut script = Shaper::new_hb_icu_script_run_iterator(utf8_text);
    // for multiple fonts
    let mut font_runs = Shaper::new_font_mgr_run_iterator(utf8_text, font, Some(font_mgr));

    shaper.shape_with_iterators(
        utf8_text,
        &mut font_runs,
        &mut bidi,
        &mut script,
        &mut lang,
        width,
        &mut TextBlobBuilderRunHandler::new(),
    );*/

    shaper.shape(text, font, true, width, &mut handler);

    let blob = handler.make_blob();
    let final_rect = handler.final_rect(font);
    let line_end_offsets = handler.line_end_offsets;

    let mut line_break_offsets = vec![];
    if line_end_offsets.len() > 1 {
        line_break_offsets = line_end_offsets;
        line_break_offsets.pop();
    }
    glyph_bounds.push(final_rect);
    let word_breaks = (0..text.len()).map(|i| text.is_char_boundary(i)).collect();

    ShapeResult {
        blob,
        line_break_offsets,
        glyph_bounds,
        word_breaks,
        vertical_advance,
    }
}
