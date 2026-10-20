use crate::savers::Saver;

pub const BG: [f32; 4] = hex(0x12110e, 1.0);
pub const RAISE: [f32; 4] = hex(0x1b1914, 1.0);
pub const INK: [u8; 4] = [0xec, 0xe7, 0xdc, 255];
pub const MUTED: [u8; 4] = [0xae, 0xa8, 0x9a, 255];
pub const INK_SOFT: [u8; 4] = [0xec, 0xe7, 0xdc, 220];
pub const BRAND: [u8; 4] = [0xec, 0xe7, 0xdc, 158];
pub const AMBER: [u8; 4] = [0xe0, 0xa4, 0x5a, 255];

pub const fn hex(rgb: u32, a: f32) -> [f32; 4] {
    [
        ((rgb >> 16) & 255) as f32 / 255.0,
        ((rgb >> 8) & 255) as f32 / 255.0,
        (rgb & 255) as f32 / 255.0,
        a,
    ]
}

fn ink() -> [u8; 4] {
    crate::theme::ink()
}
fn muted() -> [u8; 4] {
    crate::theme::muted()
}
fn ink_soft() -> [u8; 4] {
    crate::theme::ink_soft()
}
fn brand() -> [u8; 4] {
    crate::theme::brand()
}
fn accent() -> [u8; 4] {
    crate::theme::accent()
}
fn line() -> [f32; 4] {
    crate::theme::line()
}
fn hover_ink() -> [f32; 4] {
    crate::theme::hover()
}
fn raise() -> [f32; 4] {
    crate::theme::raise()
}
fn scrim() -> [f32; 4] {
    crate::theme::scrim()
}
fn mark() -> [f32; 4] {
    crate::theme::mark()
}

/// A size written for a shell text size of 12, in the current theme's scale.
fn s(size: f32) -> f32 {
    crate::theme::px(size)
}

#[derive(Clone)]
pub struct TextRun {
    pub text: String,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub size: f32,
    pub line_height: f32,
    pub color: [u8; 4],
    pub serif: bool,
    pub italic: bool,
    /// Extra letter spacing in ems. The shaper multiplies this by the font size.
    pub tracking: f32,
    pub clip: Option<[f32; 4]>,
    /// The whole run is one code block. Inline code stays in `text` between backticks.
    pub block: bool,
    /// One line, centered in `w`. The shelf bar uses this so the hyphen does not wrap.
    pub center: bool,
    /// One line, aligned to the right edge of `w`. The card pill uses this.
    pub align_right: bool,
    /// The percent pill on a card. It can take the highlight colors and extra padding.
    pub badge: bool,
}

#[derive(Clone, Copy)]
pub struct CardGeom {
    pub index: usize,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    /// The picture. Shelf pictures are square.
    pub h: f32,
    /// Room under the picture. The shelf shows the picture alone, so this is 0.
    pub tail: f32,
}

impl CardGeom {
    pub fn span(self) -> f32 {
        self.h + self.tail
    }
}

/// Width divided by height for a monitor. A missing size uses 16:9.
pub fn screen_aspect(width: f32, height: f32) -> f32 {
    if width.is_finite() && height.is_finite() && width >= 1.0 && height >= 1.0 {
        (width / height).clamp(0.25, 4.0)
    } else {
        16.0 / 9.0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct PaintRect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub color: [f32; 4],
    pub radius: f32,
    pub mode: f32,
}

pub struct Layout {
    pub cards: Vec<CardGeom>,
    pub texts: Vec<TextRun>,
    pub rects: Vec<PaintRect>,
    /// Vertical offset applied to this layout, already clamped to the content.
    pub scroll: f32,
    /// x, y, width, height of the bottom bar. A click here opens the guidelines.
    pub bar: [f32; 4],
}

impl Layout {
    pub fn hit(&self, x: f32, y: f32) -> Option<usize> {
        if self.bar_contains(x, y) {
            return None;
        }
        self.cards.iter().rev().find_map(|card| {
            (x >= card.x && y >= card.y && x < card.x + card.w && y < card.y + card.span())
                .then_some(card.index)
        })
    }

    pub fn bar_contains(&self, x: f32, y: f32) -> bool {
        let [bx, by, bw, bh] = self.bar;
        x >= bx && y >= by && x < bx + bw && y < by + bh
    }
}

pub struct HelpView {
    pub texts: Vec<TextRun>,
    /// Vertical offset applied to this view, already clamped to the content.
    pub scroll: f32,
}

/// The shelf keeps five columns. The cards take the width that leaves.
pub fn gallery_cols(_win_w: f32) -> usize {
    5
}

#[derive(Clone, Copy)]
pub enum Move {
    Previous,
    Next,
    Up,
    Down,
}

/// One step through the shelf. Left and right walk the name order and wrap.
/// Up and down move a row and stay put at the edge.
pub fn step_index(index: usize, count: usize, cols: usize, movement: Move) -> usize {
    if count == 0 {
        return 0;
    }
    let index = index.min(count - 1);
    let cols = cols.max(1);
    match movement {
        Move::Previous => (index as isize - 1).rem_euclid(count as isize) as usize,
        Move::Next => (index + 1) % count,
        Move::Up => {
            if index >= cols {
                index - cols
            } else {
                index
            }
        }
        Move::Down => {
            let next = index + cols;
            if next < count {
                next
            } else {
                index
            }
        }
    }
}

pub fn gallery(
    savers: &[Saver],
    _shader_dir: &str,
    win_w: f32,
    win_h: f32,
    scroll: f32,
    hover: Option<usize>,
    detail: Option<u8>,
    pace: Option<u32>,
    aspect: f32,
) -> Layout {
    let cols = gallery_cols(win_w);
    let margin = if win_w <= 760.0 { 16.0 } else { 24.0 };
    let content_w = (win_w - margin * 2.0).max(1.0);
    let x0 = (win_w - content_w) * 0.5;
    let gap = 14.4;
    let card_w = ((content_w - gap * (cols as f32 - 1.0)) / cols as f32).max(1.0);
    // The picture is square. Opening a card still fills the window.
    let _ = aspect;
    let picture_h = card_w;
    let card_h = picture_h;
    let bar_h = bottom_bar_height();
    let view_h = (win_h - bar_h).max(1.0);

    let order: Vec<usize> = (0..savers.len()).collect();

    let group_h = |count: usize| -> f32 {
        if count == 0 {
            return 0.0;
        }
        let rows = count.div_ceil(cols) as f32;
        rows * card_h + (rows - 1.0).max(0.0) * gap
    };
    let content_h = if savers.is_empty() {
        48.0
    } else {
        group_h(savers.len())
    };
    // A short window scrolls instead of clipping the last row. The bar stays put.
    let max_scroll = (content_h + 32.0 - view_h).max(0.0);
    let scroll = if max_scroll == 0.0 {
        0.0
    } else {
        scroll.clamp(0.0, max_scroll)
    };
    let y = if max_scroll == 0.0 {
        (view_h - content_h) * 0.5
    } else {
        18.0 - scroll
    };

    let mut texts = Vec::new();
    let rects = Vec::new();
    let mut cards = Vec::new();

    if order.is_empty() {
        texts.push(run(
            "No savers loaded.",
            x0,
            y,
            content_w,
            s(18.0),
            s(26.0),
            muted(),
            false,
            false,
            0.0,
            None,
        ));
    } else {
        place_group(
            savers, &order, x0, y, cols, card_w, picture_h, gap, hover, detail, pace, &mut cards,
            &mut texts,
        );
    }

    let bar_y = (win_h - bar_h).max(0.0);
    place_bar_label(win_w, bar_y, bar_h, &mut texts);

    Layout {
        cards,
        texts,
        rects,
        scroll,
        bar: [0.0, bar_y, win_w.max(1.0), bar_h],
    }
}

/// The guidelines and tips. `?` opens this from the shelf, and leaves it again.
pub fn help(shader_dir: &str, win_w: f32, win_h: f32, scroll: f32) -> HelpView {
    let margin = if win_w <= 760.0 { 16.0 } else { 24.0 };
    let content_w = (win_w - margin * 2.0).max(1.0);
    let x0 = (win_w - content_w) * 0.5;
    let title_size = (win_w * 0.046).clamp(s(38.0), s(70.0));
    let lede =
        "Open a card and let it run. `F` fills the screen. `Esc` or `Q` brings you back to the shelf.";
    let lede_h = estimate_lines(lede, content_w, s(LEDE_SIZE)).max(1) as f32 * s(LEDE_LH);
    let notes_h = notes_block_height(shader_dir, content_w);
    let hint = "`?`, `Esc`, or `Q` brings you back to the shelf.";
    let hint_h = estimate_lines(hint, content_w, s(16.5)).max(1) as f32 * s(24.0);
    let header_h = title_size + s(16.0) + lede_h + s(28.0);
    let content_h = header_h + notes_h + s(28.0) + hint_h;
    let max_scroll = (content_h + 32.0 - win_h).max(0.0);
    let scroll = if max_scroll == 0.0 {
        0.0
    } else {
        scroll.clamp(0.0, max_scroll)
    };
    let mut y = if max_scroll == 0.0 {
        (win_h - content_h) * 0.5
    } else {
        18.0 - scroll
    };

    let mut texts = Vec::new();
    texts.push(run(
        "Save the Screen",
        x0,
        y,
        content_w,
        title_size,
        title_size * 0.95,
        ink(),
        true,
        true,
        0.0,
        None,
    ));
    texts.push(run(
        lede,
        x0,
        y + title_size + s(16.0),
        content_w,
        s(LEDE_SIZE),
        s(LEDE_LH),
        muted(),
        false,
        false,
        0.0,
        None,
    ));
    y += header_h;
    place_gallery_notes(shader_dir, x0, y, content_w, &mut texts);
    texts.push(run(
        hint,
        x0,
        y + notes_h + s(28.0),
        content_w,
        s(16.5),
        s(24.0),
        ink_soft(),
        false,
        false,
        0.0,
        None,
    ));
    HelpView { texts, scroll }
}

/// A step under two note lines. The label is one smaller line in the middle.
pub fn bottom_bar_height() -> f32 {
    s(40.0)
}

const BAR_LABEL: &str = "? - info";

fn place_bar_label(win_w: f32, bar_y: f32, bar_h: f32, texts: &mut Vec<TextRun>) {
    let size = crate::theme::body_small();
    let line_height = (size * 1.35).max(size + 2.0);
    let mut label = run(
        BAR_LABEL,
        0.0,
        bar_y + (bar_h - line_height) * 0.5,
        win_w.max(1.0),
        size,
        line_height,
        crate::theme::bar_ink(),
        false,
        false,
        0.0,
        None,
    );
    label.center = true;
    texts.push(label);
}

/// The strip under the pictures, drawn after the cards so a scrolled row stays behind it.
pub fn bottom_bar_paint(bar: [f32; 4]) -> Vec<PaintRect> {
    let [x, y, w, h] = bar;
    vec![
        PaintRect {
            x,
            y,
            w,
            h,
            color: crate::theme::bar_bg(),
            radius: 0.0,
            mode: 0.0,
        },
        PaintRect {
            x,
            y,
            w,
            h: 1.0,
            color: line(),
            radius: 0.0,
            mode: 0.0,
        },
    ]
}

struct GalleryNote {
    heading: &'static str,
    body: String,
}

const NOTE_HEAD: f32 = 16.0;
const NOTE_AFTER_HEAD: f32 = 6.0;
const NOTE_BODY_SIZE: f32 = 16.5;
const NOTE_BODY_LH: f32 = 25.0;
const NOTE_BLOCK_GAP: f32 = 22.0;

/// The pad under a note, plus one blank line, so the description rows stay apart.
fn note_row_gap() -> f32 {
    NOTE_BLOCK_GAP + s(NOTE_BODY_LH)
}
const NOTE_COL_GAP: f32 = 36.0;
const NOTE_COLUMNS: usize = 3;
/// Below this, the five notes stack so a column stays readable.
const NOTE_MIN_COL: f32 = 300.0;
const LEDE_SIZE: f32 = 18.0;
const LEDE_LH: f32 = 27.0;

/// Reading order. Wide windows lay the first three on one row and the last two on the next.
fn gallery_notes(shader_dir: &str) -> Vec<GalleryNote> {
    vec![
        GalleryNote {
            heading: "ON THE SHELF",
            body: "Each card is a square. Rest the pointer on it and it plays. Move away and it keeps that frame. `Arrows`, or `h j k l`, move through the cards. `Enter` opens the highlighted one. `Del` removes it, after you confirm.".into(),
        },
        GalleryNote {
            heading: "A CORNER MARK",
            body: "A corner mark means a channel is reading a `256` noise picture, or the shader needs a small edit. Hover the card and it says which. Open it for the line to change.".into(),
        },
        GalleryNote {
            heading: "WHILE IT RUNS",
            body: "`Left` and `Right`, or `h` and `l`, switch to the previous or next saver. `F` fills the screen. `Esc` or `Q` returns to the shelf. From the shelf, `Esc` or `Q` quits.".into(),
        },
        GalleryNote {
            heading: "PICTURE SIZE",
            body: "Keys `1` through `0` set the picture from 10% to the full window. On the shelf they set the highlighted card. While a saver runs, `j` and `k` step it by 10. `PgUp` and `PgDn` step the pace from 30 fps up to this screen. It opens at 50% and 30 fps, and both are remembered.".into(),
        },
        GalleryNote {
            heading: "ADD A SAVER",
            body: format!(
                "Save a Shadertoy `.frag` file in `{shader_dir}`. Start the app again and it joins the shelf, sorted by name."
            ),
        },
    ]
}

fn note_column_width(width: f32) -> Option<f32> {
    let col_w = (width - NOTE_COL_GAP * (NOTE_COLUMNS as f32 - 1.0)) / NOTE_COLUMNS as f32;
    (col_w >= NOTE_MIN_COL).then_some(col_w)
}

fn note_block_height(body: &str, width: f32) -> f32 {
    let lines = estimate_lines(body, width, s(NOTE_BODY_SIZE)).max(1) as f32;
    s(NOTE_HEAD) + s(NOTE_AFTER_HEAD) + lines * s(NOTE_BODY_LH)
}

fn column_height(notes: &[GalleryNote], width: f32) -> f32 {
    if notes.is_empty() {
        return 0.0;
    }
    let mut height = 0.0;
    for (index, note) in notes.iter().enumerate() {
        height += note_block_height(&note.body, width);
        if index + 1 != notes.len() {
            height += note_row_gap();
        }
    }
    height
}

fn notes_block_height(shader_dir: &str, width: f32) -> f32 {
    let notes = gallery_notes(shader_dir);
    let Some(col_w) = note_column_width(width) else {
        return column_height(&notes, width);
    };
    let row = |notes: &[GalleryNote]| {
        notes
            .iter()
            .map(|note| note_block_height(&note.body, col_w))
            .fold(0.0_f32, f32::max)
    };
    row(&notes[..3]) + note_row_gap() + row(&notes[3..])
}

fn place_gallery_notes(shader_dir: &str, x: f32, y: f32, width: f32, texts: &mut Vec<TextRun>) {
    let notes = gallery_notes(shader_dir);
    let Some(col_w) = note_column_width(width) else {
        place_note_column(&notes, x, y, width, texts);
        return;
    };
    let mut row_bottom = y;
    for (index, note) in notes[..3].iter().enumerate() {
        let end = place_one_note(
            note,
            x + index as f32 * (col_w + NOTE_COL_GAP),
            y,
            col_w,
            texts,
        );
        row_bottom = row_bottom.max(end);
    }
    let y2 = row_bottom + note_row_gap();
    for (index, note) in notes[3..].iter().enumerate() {
        place_one_note(
            note,
            x + index as f32 * (col_w + NOTE_COL_GAP),
            y2,
            col_w,
            texts,
        );
    }
}

fn place_note_column(
    notes: &[GalleryNote],
    x: f32,
    mut y: f32,
    width: f32,
    texts: &mut Vec<TextRun>,
) -> f32 {
    for (index, note) in notes.iter().enumerate() {
        y = place_one_note(note, x, y, width, texts);
        if index + 1 != notes.len() {
            y += note_row_gap();
        }
    }
    y
}

fn place_one_note(note: &GalleryNote, x: f32, y: f32, width: f32, texts: &mut Vec<TextRun>) -> f32 {
    texts.push(run(
        note.heading,
        x,
        y,
        width,
        s(12.5),
        s(NOTE_HEAD),
        accent(),
        false,
        false,
        0.16,
        None,
    ));
    let body_y = y + s(NOTE_HEAD) + s(NOTE_AFTER_HEAD);
    push_paragraph(
        texts,
        &note.body,
        x,
        body_y,
        width,
        s(NOTE_BODY_SIZE),
        s(NOTE_BODY_LH),
        muted(),
        false,
    )
}

fn place_group(
    savers: &[Saver],
    indices: &[usize],
    x0: f32,
    mut y: f32,
    cols: usize,
    card_w: f32,
    picture_h: f32,
    gap: f32,
    hover: Option<usize>,
    detail: Option<u8>,
    pace: Option<u32>,
    cards: &mut Vec<CardGeom>,
    texts: &mut Vec<TextRun>,
) -> f32 {
    let mut placed = 0;
    while placed < indices.len() {
        let row = (indices.len() - placed).min(cols);
        for col in 0..row {
            let index = indices[placed + col];
            let saver = &savers[index];
            let x = x0 + col as f32 * (card_w + gap);
            cards.push(CardGeom {
                index,
                x,
                y,
                w: card_w,
                h: picture_h,
                tail: 0.0,
            });
            // The percent stays on the picture, in the lower right. The name and the blurb do not.
            if hover == Some(index) && saver.hold.is_none() {
                if let Some(percent) = detail {
                    let label = match pace {
                        Some(pace) => format!("`{percent}% · {pace} fps`"),
                        None => format!("`{percent}%`"),
                    };
                    let inset = 16.0;
                    let size = s(14.5);
                    let line_height = s(18.0);
                    // The pill grows a few pixels past the line. Lift it so that stays inside.
                    let lift = s(4.0);
                    let mut badge = run(
                        &label,
                        x + inset,
                        y + picture_h - inset - line_height - lift,
                        (card_w - inset * 2.0).max(72.0),
                        size,
                        line_height,
                        accent(),
                        false,
                        false,
                        0.06,
                        Some([x, y, card_w, picture_h]),
                    );
                    badge.align_right = true;
                    badge.badge = true;
                    texts.push(badge);
                }
            }
        }
        placed += row;
        y += picture_h + gap;
    }
    y - gap
}

pub fn hud(
    savers: &[Saver],
    index: usize,
    win_w: f32,
    win_h: f32,
    detail: u8,
    pace: u32,
) -> Vec<TextRun> {
    let saver = &savers[index];
    let left = 28.0;
    let width = (win_w - left * 2.0).max(160.0);
    let hint = "`F` fullscreen  ·  `h l` switch  ·  `j k` or `1–0` detail  ·  `PgUp` `PgDn` pace  ·  `Esc` or `Q` back";
    let hint_size = s(16.5);
    let hint_lh = s(24.0);
    let hint_lines = estimate_lines(hint, width, hint_size).max(1) as f32;
    let hint_y = win_h - s(18.0) - hint_lines * hint_lh;
    let title_size = (win_w * 0.04).clamp(s(32.0), s(50.0));
    let title_y = hint_y - s(12.0) - title_size;
    let brand_y = title_y - s(24.0);
    vec![
        run(
            &format!(
                "SAVE THE SCREEN  ·  {} OF {}  ·  {detail}%  ·  {pace} fps",
                index + 1,
                savers.len()
            ),
            left,
            brand_y,
            width,
            s(12.5),
            s(16.0),
            brand(),
            false,
            false,
            0.16,
            None,
        ),
        run(
            &saver.name,
            left,
            title_y,
            width,
            title_size,
            title_size,
            ink(),
            true,
            true,
            0.0,
            None,
        ),
        run(
            hint,
            left,
            hint_y,
            width,
            hint_size,
            hint_lh,
            ink_soft(),
            false,
            false,
            0.0,
            None,
        ),
    ]
}

/// The still note for a shader this app could not compile. Paragraphs in `hold.page`
/// are separated by a blank line. Each run stays within the text buffer's line window.
pub fn hold_page(saver: &Saver, win_w: f32, win_h: f32) -> Vec<TextRun> {
    let Some(hold) = &saver.hold else {
        return Vec::new();
    };
    let left = 48.0_f32.min(win_w * 0.08).max(16.0);
    let width = (win_w - left * 2.0).clamp(160.0, 720.0);
    let mut y = 48.0;
    let mut texts = Vec::new();
    texts.push(run(
        "A SMALL EDIT",
        left,
        y,
        width,
        s(12.5),
        s(16.0),
        accent(),
        false,
        false,
        0.16,
        None,
    ));
    y += s(28.0);
    let title_size = (win_w * 0.04).clamp(s(28.0), s(48.0));
    texts.push(run(
        &saver.name,
        left,
        y,
        width,
        title_size,
        title_size,
        ink(),
        true,
        true,
        0.0,
        None,
    ));
    y += title_size + 20.0;
    for paragraph in hold.page.split("\n\n") {
        let raw = paragraph.trim();
        if raw.is_empty() {
            continue;
        }
        let (text, block) = if let Some(code) = fenced_block(raw) {
            (code, true)
        } else {
            (raw, is_code(raw))
        };
        if text.is_empty() {
            continue;
        }
        let size = if block { s(17.5) } else { s(19.5) };
        let line_height = if block { s(24.0) } else { s(28.0) };
        let color = if block { ink() } else { ink_soft() };
        y = push_paragraph(
            &mut texts,
            text,
            left,
            y,
            width,
            size,
            line_height,
            color,
            block,
        );
        y += if block { 18.0 } else { 14.0 };
    }
    if !saver.origin.is_empty() && !saver.origin.starts_with("built-in") {
        y = push_paragraph(
            &mut texts,
            &format!("The file is `{}`.", saver.origin),
            left,
            y,
            width,
            s(17.5),
            s(25.0),
            muted(),
            false,
        );
        y += 14.0;
    }
    let hint_y = if y + 28.0 < win_h - 40.0 {
        win_h - 40.0
    } else {
        y + 8.0
    };
    texts.push(run(
        "`Esc` or `Q` brings you back to the shelf.",
        left,
        hint_y,
        width,
        s(16.5),
        s(24.0),
        ink_soft(),
        false,
        false,
        0.0,
        None,
    ));
    texts
}

fn is_code(text: &str) -> bool {
    if text.contains('\n') {
        return text.contains("mat2") || text.contains('{') || text.contains('}');
    }
    text.contains("mat2(") && !text.contains("write ")
}

/// A markdown fence around a shader line. The fences are not drawn.
fn fenced_block(text: &str) -> Option<&str> {
    let rest = text.trim().strip_prefix("```\n")?;
    rest.strip_suffix("\n```")
}

/// Splits markdown inline code. An unclosed backtick stays as written.
pub(crate) fn inline_spans(text: &str) -> Vec<(&str, bool)> {
    let mut spans = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find('`') {
        if start > 0 {
            spans.push((&rest[..start], false));
        }
        let after = &rest[start + 1..];
        if let Some(end) = after.find('`') {
            if end > 0 {
                spans.push((&after[..end], true));
            }
            rest = &after[end + 1..];
        } else {
            spans.push((&rest[start..], false));
            rest = "";
            break;
        }
    }
    if !rest.is_empty() {
        spans.push((rest, false));
    }
    if spans.is_empty() {
        spans.push(("", false));
    }
    spans
}

/// Visible width, in roughly average characters. Backticks are not drawn.
/// Each inline span adds a little for the chip padding.
fn span_units(text: &str) -> usize {
    inline_spans(text)
        .into_iter()
        .map(|(slice, code)| {
            let count = slice.chars().count();
            if code {
                // Room for the side margin and the padding inside a code pill.
                count + 5
            } else {
                count
            }
        })
        .sum()
}

/// Glyphon keeps about eight lines per run. Hand it shorter pieces.
fn push_paragraph(
    texts: &mut Vec<TextRun>,
    text: &str,
    x: f32,
    mut y: f32,
    width: f32,
    size: f32,
    line_height: f32,
    color: [u8; 4],
    block: bool,
) -> f32 {
    let mut rest = text;
    while !rest.is_empty() {
        let (chunk, next) = take_lines(rest, width, size, 6, block);
        if chunk.is_empty() {
            break;
        }
        let lines = estimate_lines_block(chunk, width, size, block).max(1);
        let mut piece = run(
            chunk,
            x,
            y,
            width,
            size,
            line_height,
            color,
            false,
            false,
            0.0,
            None,
        );
        piece.block = block;
        texts.push(piece);
        y += lines as f32 * line_height;
        if next == rest {
            break;
        }
        rest = next.trim_start_matches('\n');
    }
    y
}

fn take_lines(text: &str, width: f32, size: f32, max_rows: usize, block: bool) -> (&str, &str) {
    let mut rows = 0;
    let mut end = 0;
    for line in text.split_inclusive('\n') {
        let wrapped = estimate_lines_block(line.trim_end_matches('\n'), width, size, block).max(1);
        if rows > 0 && rows + wrapped > max_rows {
            break;
        }
        rows += wrapped;
        end += line.len();
        if rows >= max_rows {
            break;
        }
    }
    if end == 0 {
        return (text, "");
    }
    (&text[..end], &text[end..])
}

fn estimate_lines(text: &str, width: f32, size: f32) -> usize {
    estimate_lines_block(text, width, size, false)
}

fn estimate_lines_block(text: &str, width: f32, size: f32, block: bool) -> usize {
    let factor = if block { 0.62 } else { 0.55 };
    let per = ((width / (size * factor)).floor() as usize).max(12);
    let mut rows = 0;
    for line in text.lines() {
        let count = if block {
            line.chars().count()
        } else {
            span_units(line)
        };
        rows += if count == 0 { 1 } else { count.div_ceil(per) };
    }
    if rows == 0 {
        1
    } else {
        rows
    }
}

/// A small mark in the top-right of a card that has something to say.
/// Drawn after the preview, so `mode` stays 0 and the caller appends these late.
pub fn note_marks(cards: &[CardGeom], savers: &[Saver]) -> Vec<PaintRect> {
    let mut marks = Vec::new();
    for card in cards {
        let Some(saver) = savers.get(card.index) else {
            continue;
        };
        if saver.note.is_empty() && saver.hold.is_none() {
            continue;
        }
        let d = 9.0;
        let x = card.x + card.w - 14.0 - d;
        let y = card.y + 14.0;
        marks.push(PaintRect {
            x: x - 1.5,
            y: y - 1.5,
            w: d + 3.0,
            h: d + 3.0,
            color: scrim(),
            radius: 20.0,
            mode: 0.0,
        });
        marks.push(PaintRect {
            x,
            y,
            w: d,
            h: d,
            color: mark(),
            radius: 20.0,
            mode: 0.0,
        });
    }
    marks
}

pub fn card_paints(cards: &[CardGeom], hover: Option<usize>) -> Vec<PaintRect> {
    let mut rects = Vec::with_capacity(cards.len() * 2);
    for card in cards {
        let hot = hover == Some(card.index);
        rects.push(PaintRect {
            x: card.x,
            y: card.y,
            w: card.w,
            h: card.h,
            color: if hot { hover_ink() } else { line() },
            radius: 14.0,
            mode: 0.0,
        });
        rects.push(PaintRect {
            x: card.x + 1.0,
            y: card.y + 1.0,
            w: card.w - 2.0,
            h: card.h - 2.0,
            color: raise(),
            radius: 13.0,
            mode: 0.0,
        });
    }
    rects
}

#[derive(Clone)]
pub struct ConfirmCopy {
    pub heading: String,
    pub body: String,
}

pub struct ConfirmOverlay {
    pub rects: Vec<PaintRect>,
    pub texts: Vec<TextRun>,
    /// x, y, width, height of the panel. A click outside it keeps the shader.
    pub panel: [f32; 4],
}

/// A centered question over a dimmed shelf.
pub fn confirm_overlay(copy: &ConfirmCopy, win_w: f32, win_h: f32) -> ConfirmOverlay {
    let pad = s(28.0);
    let width = 560.0_f32.min((win_w - 48.0).max(280.0));
    let text_w = (width - pad * 2.0).max(120.0);
    let heading_size = s(26.0);
    let mut texts = Vec::new();
    let mut y = 0.0;
    texts.push(run(
        &copy.heading,
        0.0,
        y,
        text_w,
        heading_size,
        heading_size * 0.95,
        ink(),
        true,
        true,
        0.0,
        None,
    ));
    y += heading_size + s(14.0);
    y = push_paragraph(
        &mut texts,
        &copy.body,
        0.0,
        y,
        text_w,
        s(17.5),
        s(26.0),
        muted(),
        false,
    );
    let height = y + pad * 2.0;
    let x = ((win_w - width) * 0.5).max(12.0);
    let origin_y = ((win_h - height) * 0.38).max(16.0);
    for text in &mut texts {
        text.x += x + pad;
        text.y += origin_y + pad;
    }
    let panel = [x, origin_y, width, height];
    let rects = vec![
        PaintRect {
            x: 0.0,
            y: 0.0,
            w: win_w.max(1.0),
            h: win_h.max(1.0),
            color: scrim(),
            radius: 0.0,
            mode: 0.0,
        },
        PaintRect {
            x,
            y: origin_y,
            w: width,
            h: height,
            color: raise(),
            radius: 16.0,
            mode: 0.0,
        },
    ];
    ConfirmOverlay {
        rects,
        texts,
        panel,
    }
}

fn run(
    text: &str,
    x: f32,
    y: f32,
    w: f32,
    size: f32,
    line_height: f32,
    color: [u8; 4],
    serif: bool,
    italic: bool,
    tracking: f32,
    clip: Option<[f32; 4]>,
) -> TextRun {
    TextRun {
        text: text.to_string(),
        x,
        y,
        w,
        size,
        line_height,
        color,
        serif,
        italic,
        tracking,
        clip,
        block: false,
        center: false,
        align_right: false,
        badge: false,
    }
}

/// Whole-frame drift, matching the page's 97s CSS wander. Values are CSS pixels,
/// positive x to the right and positive y down.
pub fn wander(time_s: f32) -> (f32, f32) {
    const KEYS: [(f32, f32, f32); 5] = [
        (0.00, 0.0, 0.0),
        (0.20, 10.0, -8.0),
        (0.45, -9.0, 7.0),
        (0.70, 7.0, 10.0),
        (1.00, 0.0, 0.0),
    ];
    let t = time_s.rem_euclid(97.0) / 97.0;
    let mut i = 0;
    while i + 1 < KEYS.len() - 1 && t > KEYS[i + 1].0 {
        i += 1;
    }
    let (t0, x0, y0) = KEYS[i];
    let (t1, x1, y1) = KEYS[i + 1];
    let u = ((t - t0) / (t1 - t0)).clamp(0.0, 1.0);
    let s = u * u * (3.0 - 2.0 * u);
    (x0 + (x1 - x0) * s, y0 + (y1 - y0) * s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steps_walk_the_grid() {
        assert_eq!(step_index(0, 10, 5, Move::Next), 1);
        assert_eq!(step_index(4, 10, 5, Move::Next), 5);
        assert_eq!(step_index(0, 10, 5, Move::Previous), 9);
        assert_eq!(step_index(9, 10, 5, Move::Next), 0);
        assert_eq!(step_index(6, 10, 5, Move::Up), 1);
        assert_eq!(step_index(2, 10, 5, Move::Up), 2);
        assert_eq!(step_index(3, 10, 5, Move::Down), 8);
        assert_eq!(step_index(8, 10, 5, Move::Down), 8);
        assert_eq!(step_index(0, 0, 5, Move::Next), 0);
    }

    fn sample() -> crate::savers::Saver {
        crate::savers::Saver {
            id: "veil".into(),
            name: "Veil".into(),
            blurb: "A quiet cloth.".into(),
            source: String::new(),
            origin: String::new(),
            note: String::new(),
            hold: None,
        }
    }

    #[test]
    fn a_held_card_keeps_its_mark_and_says_why() {
        let mut saver = sample();
        saver.hold = Some(crate::savers::Hold {
            card: "This shader's turn is written too briefly for this app. Open the card for the line to change.".into(),
            page: "Line 110\n\nreturn mat2(c.xy, c.zw);".into(),
        });
        let savers = [saver];
        let laid = gallery(
            &savers,
            "/shaders",
            1440.0,
            900.0,
            0.0,
            Some(0),
            Some(50),
            None,
            16.0 / 9.0,
        );
        let joined = laid
            .texts
            .iter()
            .map(|text| text.text.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(!joined.contains("too briefly"));
        assert!(!joined.contains("quiet cloth"));
        assert!(!joined.contains("Veil"));
        assert!(laid.texts.iter().all(|text| text.text != "50%"));
        assert!(!note_marks(&laid.cards, &savers).is_empty());

        let page = hold_page(&savers[0], 1440.0, 900.0);
        let page_text = page
            .iter()
            .map(|text| text.text.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(page_text.contains("mat2(c.xy, c.zw)"));
        assert!(page
            .iter()
            .any(|text| text.block && text.text.contains("mat2")));
        assert!(page_text.contains("`Esc` or `Q`"));
    }

    #[test]
    fn cards_are_square() {
        let savers = [sample()];
        let wide = gallery(
            &savers,
            "/shaders",
            1440.0,
            900.0,
            0.0,
            None,
            None,
            None,
            16.0 / 9.0,
        );
        let card = wide.cards[0];
        assert!((card.w - card.h).abs() < 1.0);
        assert!(card.tail.abs() < 0.01);
        assert!(wide.hit(card.x + 8.0, card.y + 8.0).is_some());
        assert!(wide.hit(card.x + 8.0, card.y + card.h + 4.0).is_none());
        let [_, bar_y, _, _] = wide.bar;
        assert!(card.y + card.span() <= bar_y + 0.5);
        assert!(wide.bar_contains(720.0, bar_y + 4.0));
        assert!(wide.hit(720.0, bar_y + 4.0).is_none());
        assert!(wide.texts.iter().all(|text| text.text != "Veil"));
        assert!(wide
            .texts
            .iter()
            .all(|text| !text.text.contains("quiet cloth")));

        let tall = gallery(
            &savers,
            "/shaders",
            800.0,
            1400.0,
            0.0,
            None,
            None,
            None,
            9.0 / 16.0,
        );
        let card = tall.cards[0];
        assert!((card.w - card.h).abs() < 1.0);

        assert!((screen_aspect(3840.0, 2160.0) - 16.0 / 9.0).abs() < 1e-4);
        assert!((screen_aspect(2160.0, 3840.0) - 9.0 / 16.0).abs() < 1e-4);
        assert!((screen_aspect(0.0, 100.0) - 16.0 / 9.0).abs() < 1e-4);
    }

    fn note(texts: &[TextRun], heading: &str) -> TextRun {
        texts
            .iter()
            .find(|text| text.text == heading)
            .cloned()
            .unwrap_or_else(|| panic!("missing {heading}"))
    }

    #[test]
    fn help_notes_are_grouped_for_anyone() {
        let shelf = gallery(
            &[sample()],
            "/shaders",
            1440.0,
            900.0,
            0.0,
            None,
            None,
            None,
            16.0 / 9.0,
        );
        assert!(shelf.texts.iter().all(|text| text.text != "ON THE SHELF"));
        assert!(shelf.texts.iter().any(|text| text.text == "? - info"));
        let label = shelf
            .texts
            .iter()
            .find(|text| text.text == "? - info")
            .unwrap();
        assert!((label.x + label.w * 0.5 - 720.0).abs() < 1.0);
        let [_, bar_y, _, bar_h] = shelf.bar;
        assert!((label.y + label.line_height * 0.5 - (bar_y + bar_h * 0.5)).abs() < 1.0);
        assert!((bar_y + bar_h - 900.0).abs() < 1.0);
        assert!((bar_h - bottom_bar_height()).abs() < 0.1);
        assert!(bar_h < s(50.0));
        assert!((label.size - crate::theme::body_small()).abs() < 0.1);
        assert!(label.center);
        assert!(label.w > 400.0);

        let wide = help("/shaders", 1440.0, 900.0, 0.0);
        let joined = wide
            .texts
            .iter()
            .map(|text| text.text.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(joined.contains("Open a card and let it run."));
        assert!(joined.contains("`Del`"));
        assert!(joined.contains("`?`, `Esc`, or `Q`"));
        assert!(joined.contains("/shaders"));
        assert!(!joined.contains("IPS"));
        assert!(!joined.contains("afterimage"));
        assert!(!joined.contains("ghost"));

        let shelf = note(&wide.texts, "ON THE SHELF");
        let mark = note(&wide.texts, "A CORNER MARK");
        let running = note(&wide.texts, "WHILE IT RUNS");
        let size = note(&wide.texts, "PICTURE SIZE");
        let add = note(&wide.texts, "ADD A SAVER");
        let col = mark.x - shelf.x;
        assert!(col > 40.0, "three columns need a gap, got {col}");
        assert!((running.x - mark.x - col).abs() < 2.0);
        assert!((mark.y - shelf.y).abs() < 2.0);
        assert!((running.y - shelf.y).abs() < 2.0);
        assert!((size.x - shelf.x).abs() < 1.0);
        assert!((add.x - mark.x).abs() < 2.0);
        assert!((add.y - size.y).abs() < 2.0);
        assert!(size.y > shelf.y + 20.0);
        assert!((shelf.w - mark.w).abs() < 2.0);
        assert!((shelf.w - running.w).abs() < 2.0);
        assert!((shelf.w - size.w).abs() < 2.0);
        assert!((shelf.w - add.w).abs() < 2.0);
        assert!(
            add.x + add.w < running.x + 8.0,
            "row two stays left of the third column"
        );

        let narrow = help("/shaders", 800.0, 1400.0, 0.0);
        let shelf = note(&narrow.texts, "ON THE SHELF");
        let mark = note(&narrow.texts, "A CORNER MARK");
        let running = note(&narrow.texts, "WHILE IT RUNS");
        let size = note(&narrow.texts, "PICTURE SIZE");
        let add = note(&narrow.texts, "ADD A SAVER");
        assert!((mark.x - shelf.x).abs() < 1.0);
        assert!((running.x - shelf.x).abs() < 1.0);
        assert!(mark.y > shelf.y);
        assert!(running.y > mark.y);
        assert!(size.y > running.y);
        assert!(add.y > size.y);

        for text in &wide.texts {
            if (text.size - s(NOTE_BODY_SIZE)).abs() < 0.1 {
                let lines = estimate_lines(&text.text, text.w, text.size);
                assert!(
                    lines <= 7,
                    "a note run stays inside the text window, got {lines}: {}",
                    text.text
                );
            }
        }

        let scrolled = help("/shaders", 1440.0, 420.0, 10_000.0);
        let add = note(&scrolled.texts, "ADD A SAVER");
        assert!(add.y > 0.0);
        assert!(add.y + 48.0 < 420.0);
    }

    #[test]
    fn keys_and_code_are_marked_for_the_screen() {
        let savers = [sample()];
        let laid = help("/shaders", 1440.0, 900.0, 0.0);
        let joined = laid
            .texts
            .iter()
            .map(|text| text.text.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(joined.contains("`h j k l`"));
        assert!(joined.contains("`h` and `l`"));
        assert!(joined.contains("`PgUp`"));
        assert!(joined.contains("`PgDn`"));
        assert!(joined.contains("`Enter`"));
        assert!(joined.contains("`F`"));
        assert!(joined.contains("`Esc`"));
        assert!(joined.contains("`.frag`"));
        assert!(joined.contains("`/shaders`"));
        assert!(laid.texts.iter().all(|text| !text.block));

        let hovered = gallery(
            &savers,
            "/shaders",
            1440.0,
            900.0,
            0.0,
            Some(0),
            Some(50),
            Some(30),
            16.0 / 9.0,
        );
        let badge = hovered
            .texts
            .iter()
            .find(|text| text.text == "`50% · 30 fps`")
            .expect("percent pill");
        let card = &hovered.cards[0];
        assert!(badge.badge);
        assert!(badge.align_right);
        assert!(badge.y > card.y + card.h * 0.5);
        assert!(badge.x + badge.w > card.x + card.w - 20.0);

        let hint = &hud(&savers, 0, 1440.0, 900.0, 50, 30)[2];
        assert!(hint.text.contains("`h l`"));
        assert!(hint.text.contains("`1–0`"));
        assert!(hint.text.contains("`PgUp`"));
        assert!(hint.text.contains("`PgDn`"));
        assert!(!hint.block);
        assert!(hint.size > 16.0);

        assert_eq!(
            inline_spans("or `h` and `l`."),
            vec![
                ("or ", false),
                ("h", true),
                (" and ", false),
                ("l", true),
                (".", false),
            ]
        );
        assert_eq!(
            inline_spans("press `h"),
            vec![("press ", false), ("`h", false)]
        );
        assert_eq!(
            fenced_block("```\nreturn textureLod(iChannel0, x, 0.0);\n```"),
            Some("return textureLod(iChannel0, x, 0.0);")
        );

        let mut held = sample();
        held.origin = "/shaders/iq.hell-2.frag".into();
        held.hold = Some(crate::savers::Hold {
            card: "needs a look".into(),
            page: "See `mainImage`.\n\n```\nreturn textureLod(iChannel0, x, 0.0);\n```".into(),
        });
        let page = hold_page(&held, 1440.0, 900.0);
        assert!(page.iter().all(|text| !text.text.contains("```")));
        assert!(page
            .iter()
            .any(|text| { !text.block && text.text.contains("`mainImage`") }));
        assert!(page.iter().any(|text| {
            text.block && text.text.contains("textureLod") && !text.text.contains('`')
        }));
        assert!(page
            .iter()
            .any(|text| text.text.contains("`/shaders/iq.hell-2.frag`")));
    }

    #[test]
    fn the_shelf_keeps_five_columns() {
        assert_eq!(gallery_cols(640.0), 5);
        assert_eq!(gallery_cols(1600.0), 5);
        let savers: Vec<_> = (0..6)
            .map(|index| {
                let mut saver = sample();
                saver.id = format!("s{index}");
                saver.name = format!("Saver {index}");
                saver
            })
            .collect();
        let laid = gallery(
            &savers, "/shaders", 900.0, 800.0, 0.0, None, None, None, 1.0,
        );
        assert_eq!(laid.cards.len(), 6);
        let top = laid.cards[0].y;
        assert!(
            laid.cards
                .iter()
                .take(5)
                .all(|card| (card.y - top).abs() < 1.0),
            "the first five cards share a row"
        );
        assert!(laid.cards[5].y > top + laid.cards[0].h);
        let joined = laid
            .texts
            .iter()
            .map(|text| text.text.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(!joined.contains("FULLSCREEN SAVER"));
        assert!(!joined.contains("ON THE SHELF"));
        assert!(joined.contains("? - info"));
    }

    #[test]
    fn removing_a_shader_asks_first() {
        let overlay = confirm_overlay(
            &ConfirmCopy {
                heading: "Remove Veil?".into(),
                body: "This deletes `veil.frag`.\n\n`Enter` or `Del` removes it. `Esc` keeps it."
                    .into(),
            },
            1440.0,
            900.0,
        );
        let [x, y, w, h] = overlay.panel;
        assert!(x > 0.0 && y > 0.0);
        assert!(x + w < 1440.0);
        assert!(y + h < 900.0);
        let joined = overlay
            .texts
            .iter()
            .map(|text| text.text.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(joined.contains("Remove Veil?"));
        assert!(joined.contains("`veil.frag`"));
        assert!(joined.contains("`Esc`"));
    }
}
