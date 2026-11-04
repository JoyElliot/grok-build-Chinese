//! 在原生 Windows Terminal 中验收生产渲染路径，不需要登录或模型请求。
//! cargo run -p xai-grok-pager-render --example sixel_smoke -- <制品目录> [每阶段秒数]
//! 依次观察：不透明图、同位置透明图、完全透明图回退、关闭预览。

use std::io::{Cursor, Write};
use std::path::Path;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use crossterm::execute;
use crossterm::terminal::{EnterAlternateScreen, LeaveAlternateScreen};
use image::{ImageFormat, Rgba, RgbaImage};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::widgets::{Block, Borders, Paragraph};
use xai_grok_pager_render::prompt_images::{PastedImage, PromptImagePreview};
use xai_grok_pager_render::render::draw::{
    CursorState, PagerTerminal, TermWriter, WriterDrain, draw_frame, spawn_writer_thread,
};
use xai_grok_pager_render::render::render_image_overlay;
use xai_grok_pager_render::terminal::{image as terminal_image, overlay, terminal_context};

struct ScreenGuard;

impl Drop for ScreenGuard {
    fn drop(&mut self) {
        let _ = execute!(
            std::io::stderr(),
            LeaveAlternateScreen,
            crossterm::cursor::Show
        );
        let _ = crossterm::terminal::disable_raw_mode();
    }
}

fn fixture(kind: &str, output: &Path) -> Result<PastedImage> {
    let pixels = RgbaImage::from_fn(200, 200, |x, _| match kind {
        "transparent" => Rgba([0, 0, 0, 0]),
        "partial" if x < 100 => Rgba([0, 0, 0, 0]),
        "calibration" => Rgba([255, 153, 0, 255]),
        _ if x < 100 => Rgba([255, 0, 0, 255]),
        _ => Rgba([0, 255, 0, 255]),
    });
    let mut png = Vec::new();
    pixels.write_to(&mut Cursor::new(&mut png), ImageFormat::Png)?;
    std::fs::write(output.join(format!("{kind}.png")), &png)?;
    let image = PastedImage {
        element_id: xai_ratatui_textarea::ElementId::from_raw(1),
        display_number: 1,
        mime_type: "image/png".into(),
        dimensions: Some((200, 200)),
        byte_len: png.len(),
        encoded_bytes: Some(png.into()),
        source_path: Some("fixture.png".into()),
        staged_temp_path: None,
        session_image_path: None,
        preview: PromptImagePreview::default(),
    };
    image.prepare_preview_blocking();
    Ok(image)
}

fn run(output: &Path, hold: u64) -> Result<()> {
    if terminal_image::detect_graphics_protocol() != terminal_image::GraphicsProtocol::Sixel {
        bail!("请使用原生 Windows 构建，并在 Windows Terminal 中运行此验收程序");
    }
    let (width, height) = crossterm::terminal::size()?;
    if width < 102 || height < 29 {
        bail!("验收窗口至少需要 102 列、29 行，当前为 {width}×{height}");
    }
    let calibration = fixture("calibration", output)?;
    let opaque = fixture("opaque", output)?;
    let partial = fixture("partial", output)?;
    let transparent = fixture("transparent", output)?;
    crossterm::terminal::enable_raw_mode()?;
    let _screen = ScreenGuard;
    execute!(
        std::io::stderr(),
        EnterAlternateScreen,
        crossterm::cursor::Hide
    )?;
    let (tx, sync, _events, thread) = spawn_writer_thread()?;
    let writer = TermWriter::new(tx, sync.clone())?;
    let mut terminal = PagerTerminal::new(CrosstermBackend::new(writer))?;
    let mut cursor = CursorState::new();
    let background = Color::Rgb(0, 80, 100);
    let stages = [
        ("opaque", Some(&opaque)),
        ("partial", Some(&partial)),
        ("transparent", Some(&transparent)),
        ("closed", None),
    ];
    for (stage, image) in stages {
        // 每阶段复画两帧，保留真实双缓冲和图像所有权状态，覆盖静止帧与图片切换。
        for _ in 0..2 {
            draw_frame(
                &mut terminal,
                &mut cursor,
                terminal_context(),
                |frame, _| {
                    let area = frame.area();
                    frame.render_widget(
                        Block::default().style(Style::default().bg(background)),
                        area,
                    );
                    frame.render_widget(
                        Paragraph::new(format!(
                            "PR16 Sixel smoke | {stage} | {width}x{height} | Windows Terminal"
                        ))
                        .style(Style::default().fg(Color::White).bg(background)),
                        Rect::new(2, 1, 98, 1),
                    );
                    frame.render_widget(
                        Block::default()
                            .borders(Borders::ALL)
                            .title("40x20 cells: orange must fill border")
                            .style(Style::default().fg(Color::White).bg(background)),
                        Rect::new(2, 3, 42, 22),
                    );
                    let mut post = overlay::PostFlush::default();
                    if let Some(bytes) = calibration.encoded_bytes.as_deref()
                        && let Some(pixels) = overlay::volatile_image(bytes, 40, 20, 3, 4)
                    {
                        post.append(pixels.into());
                    }
                    if let Some(image) = image {
                        if let Some(pixels) = render_image_overlay(
                            frame.buffer_mut(),
                            Rect::new(48, 3, 52, 24),
                            image,
                            background,
                            Color::White,
                            Color::White,
                        ) {
                            post.append(pixels.into());
                        }
                    } else {
                        frame.render_widget(
                            Paragraph::new("Preview closed: no old red or green pixels")
                                .style(Style::default().fg(Color::White).bg(background)),
                            Rect::new(48, 4, 52, 1),
                        );
                    }
                    (None, Some(post))
                },
            );
            if sync.wait_drained(Duration::from_secs(10))? != WriterDrain::Drained {
                bail!("终端写入超时");
            }
        }
        std::fs::write(output.join("stage.txt"), stage)?;
        std::thread::sleep(Duration::from_secs(hold));
    }
    std::fs::write(output.join("complete.txt"), "four stages rendered\n")?;
    terminal.backend_mut().flush()?;
    drop(terminal);
    thread.join_within(Duration::from_secs(2))?;
    Ok(())
}

fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let output = args
        .next()
        .context("用法: sixel_smoke <制品目录> [每阶段秒数]")?;
    let hold = args
        .next()
        .map(|s| s.parse::<u64>())
        .transpose()?
        .unwrap_or(15);
    let output = Path::new(&output);
    std::fs::create_dir_all(output)?;
    let result = run(output, hold);
    if let Err(error) = &result {
        std::fs::write(output.join("error.txt"), format!("{error:#}"))?;
    }
    result
}
