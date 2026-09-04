//! Image preview overlay for prompt image chips.
//!
//! Renders a bordered popup when the cursor is on (or right after) an image chip, or when the chip is hovered.
//! Content follows a 2×2 matrix:
//!
//! |                    | Has filepath              | No filepath                |
//! |--------------------|---------------------------|----------------------------|
//! | **Pixels available** | Image + path footer       | Image only                 |
//! | **Pixels unavailable** | Metadata + path         | Metadata only              |
//!
//! The prompt bar chip itself never shows the path (`[Image #N]`); paths appear only here.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph, Widget, Wrap};
use unicode_width::UnicodeWidthStr;

use crate::prompt_images::PastedImage;
use crate::terminal::image as terminal_image;
use crate::terminal::overlay;
use crate::util::format_bytes;

mod content;
mod geometry;

use content::{build_meta_line, format_mime, paint_path_line, truncate_path_for_overlay};

#[cfg(test)]
use geometry::ImagePlacement;
use geometry::{
    MIN_BOX_WIDTH, MIN_META_BOX_HEIGHT, MIN_PIXEL_BOX_HEIGHT, overlay_geometry, plan_image_preview,
};

#[derive(Debug)]
struct ImageOverlayRender {
    #[cfg(test)]
    image_placement: Option<ImagePlacement>,
    escapes: Option<overlay::Escapes>,
}

#[derive(Debug, Clone, Copy)]
pub struct ImageOverlayLabels {
    pub image: &'static str,
    pub format: &'static str,
    pub dimensions: &'static str,
    pub preview_unavailable: &'static str,
    pub preview_pending: &'static str,
    pub size: &'static str,
    pub path: &'static str,
    pub loading: &'static str,
}

impl Default for ImageOverlayLabels {
    fn default() -> Self {
        Self {
            image: "Image",
            format: "Format:",
            dimensions: "Dimensions:",
            preview_unavailable: "Preview unavailable",
            preview_pending: "Preview pending",
            size: "Size:",
            path: "Path:",
            loading: "Loading...",
        }
    }
}

/// Render an image preview overlay and return any post-flush pixel escapes.
pub fn render_image_overlay(
    buf: &mut Buffer,
    area: Rect,
    image: &PastedImage,
    bg: Color,
    text_fg: Color,
    border_fg: Color,
) -> Option<overlay::Escapes> {
    render_image_overlay_inner(buf, area, image, bg, text_fg, border_fg)
        .and_then(|render| render.escapes)
}

#[allow(clippy::too_many_arguments)]
pub fn render_image_overlay_with_labels(
    buf: &mut Buffer,
    area: Rect,
    image: &PastedImage,
    bg: Color,
    text_fg: Color,
    border_fg: Color,
    labels: &ImageOverlayLabels,
) -> Option<overlay::Escapes> {
    render_image_overlay_inner_with_labels(buf, area, image, bg, text_fg, border_fg, labels)
        .and_then(|render| render.escapes)
}

fn render_image_overlay_inner(
    buf: &mut Buffer,
    area: Rect,
    image: &PastedImage,
    bg: Color,
    text_fg: Color,
    border_fg: Color,
) -> Option<ImageOverlayRender> {
    render_image_overlay_inner_with_labels(
        buf,
        area,
        image,
        bg,
        text_fg,
        border_fg,
        &ImageOverlayLabels::default(),
    )
}

#[allow(clippy::too_many_arguments)]
fn render_image_overlay_inner_with_labels(
    buf: &mut Buffer,
    area: Rect,
    image: &PastedImage,
    bg: Color,
    text_fg: Color,
    border_fg: Color,
    labels: &ImageOverlayLabels,
) -> Option<ImageOverlayRender> {
    if area.width < MIN_BOX_WIDTH {
        return None;
    }

    let theme = crate::theme::Theme::current();
    let protocol = terminal_image::prompt_preview_graphics_protocol();
    let plan = plan_image_preview(image, protocol);
    let min_height = if plan.show_pixels {
        MIN_PIXEL_BOX_HEIGHT
    } else {
        MIN_META_BOX_HEIGHT
    };
    if area.height < min_height {
        return None;
    }
    let geometry = overlay_geometry(
        area,
        plan.show_pixels,
        plan.display_path.is_some(),
        image.preview_dimensions().unwrap_or((640, 480)),
    )?;
    let overlay_rect = geometry.overlay_rect;

    crate::render::color::dim_area(buf, area, theme.bg_base, 0.5);

    ratatui::widgets::Clear.render(overlay_rect, buf);
    buf.set_style(overlay_rect, Style::default().fg(text_fg).bg(bg));

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(border_fg).bg(bg))
        .style(Style::default().bg(bg));
    let inner = block.inner(overlay_rect);
    block.render(overlay_rect, buf);

    let title_text = format!(" {} #{} ", labels.image, image.display_number);
    let meta = build_meta_line(image, plan.display_path);
    let full_title =
        if UnicodeWidthStr::width(meta.as_str()) + UnicodeWidthStr::width(title_text.as_str()) + 6
            < overlay_rect.width as usize
        {
            format!("{}\u{2500} {} ", title_text, meta)
        } else {
            title_text.clone()
        };
    let title_style = Style::default()
        .fg(text_fg)
        .bg(bg)
        .add_modifier(ratatui::style::Modifier::BOLD);
    let title_width = UnicodeWidthStr::width(full_title.as_str()) as u16;
    let title_x = overlay_rect.x + (overlay_rect.width.saturating_sub(title_width)) / 2;
    buf.set_span(
        title_x,
        overlay_rect.y,
        &Span::styled(&full_title, title_style),
        title_width,
    );

    if inner.width == 0 || inner.height == 0 {
        return Some(ImageOverlayRender {
            #[cfg(test)]
            image_placement: geometry.image_placement,
            escapes: None,
        });
    }

    // Reserve the footer so a pixel placement cannot cover the path.
    let path_footer = plan.display_path.filter(|_| inner.height >= 2);
    let image_inner = if let Some(path) = path_footer {
        let footer_y = inner.y + inner.height - 1;
        paint_path_line(
            buf,
            inner.x,
            footer_y,
            inner.width,
            path,
            labels.path,
            text_fg,
            bg,
        );
        Rect {
            x: inner.x,
            y: inner.y,
            width: inner.width,
            height: inner.height.saturating_sub(1),
        }
    } else {
        inner
    };

    if !plan.show_pixels {
        let mut lines = Vec::new();
        lines.push(Line::from(format!(
            "{} {}",
            labels.format,
            format_mime(&image.mime_type)
        )));
        if let Some((w, h)) = image.preview_dimensions() {
            lines.push(Line::from(format!("{} {} x {}", labels.dimensions, w, h)));
        }
        let status = if image.preview.is_failed() {
            Some(labels.preview_unavailable)
        } else if image.preview.is_pending() && protocol.supports_images() {
            Some(labels.preview_pending)
        } else {
            None
        };
        lines.push(Line::from(status.map(str::to_owned).unwrap_or_else(|| {
            format!("{} {}", labels.size, format_bytes(image.byte_len as u64))
        })));
        // Short boxes need the path in the body because no footer fits.
        if path_footer.is_none()
            && let Some(path) = plan.display_path
        {
            lines.push(Line::from(format!(
                "{} {}",
                labels.path,
                truncate_path_for_overlay(
                    &path.display().to_string(),
                    inner
                        .width
                        .saturating_sub(UnicodeWidthStr::width(labels.path) as u16 + 1)
                        as usize,
                )
            )));
        }

        let body = if path_footer.is_some() {
            image_inner
        } else {
            inner
        };
        let paragraph = Paragraph::new(lines)
            .style(Style::default().fg(text_fg).bg(bg))
            .wrap(Wrap { trim: false });
        paragraph.render(body, buf);

        return Some(ImageOverlayRender {
            #[cfg(test)]
            image_placement: None,
            escapes: None,
        });
    }

    if image_inner.width > 0 && image_inner.height > 0 {
        use crate::render::SafeBuf;
        let loading = labels.loading;
        let lw = UnicodeWidthStr::width(loading) as u16;
        let lx = image_inner.x + image_inner.width.saturating_sub(lw) / 2;
        let ly = image_inner.y + image_inner.height / 2;
        buf.set_span_safe(
            lx,
            ly,
            &Span::styled(loading, Style::default().fg(text_fg).bg(bg)),
            lw,
        );
    }

    let escapes = geometry.image_placement.and_then(|placement| {
        let (bytes, _) = image.preview.prepared()?;
        overlay::static_image_for_protocol(
            protocol,
            bytes,
            placement.cols,
            placement.rows,
            placement.x,
            placement.y,
            image.preview.identity(),
        )
    });
    Some(ImageOverlayRender {
        #[cfg(test)]
        image_placement: geometry.image_placement,
        escapes,
    })
}

#[cfg(test)]
mod tests;
