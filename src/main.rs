#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod export;
mod palette;
mod simulation;

use eframe::egui::{self, Color32, RichText, Vec2};
use simulation::{PRESETS, Parameters, Simulation};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const BG: Color32 = Color32::from_rgb(13, 18, 26);
const PANEL: Color32 = Color32::from_rgb(20, 27, 37);
const TEXT: Color32 = Color32::from_rgb(229, 233, 233);
const MUTED: Color32 = Color32::from_rgb(145, 162, 178);
const ACCENT: Color32 = Color32::from_rgb(130, 211, 192);

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() == 2 && args[0] == "--render-preview" {
        let mut sim = Simulation::new(256, 42);
        for _ in 0..2400 {
            sim.step(PRESETS[0].2);
        }
        export::save_png(
            std::path::Path::new(&args[1]),
            sim.size,
            &palette::rgb(&sim.cells, 0, 1.0, false),
            "Gray-Scott / F=0.029 / k=0.057 / seed=42 / steps=2400",
        )?;
        return Ok(());
    }
    if !args.is_empty() {
        return Err("Usage: turing-studio [--render-preview output.png]".into());
    }
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Turing Studio · 反応拡散のアトリエ")
            .with_inner_size([1200.0, 840.0])
            .with_min_inner_size([850.0, 640.0]),
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };
    eframe::run_native(
        "Turing Studio",
        options,
        Box::new(|cc| Ok(Box::new(Studio::new(cc)))),
    )?;
    Ok(())
}

struct Studio {
    sim: Simulation,
    params: Parameters,
    preset: usize,
    palette: usize,
    contrast: f32,
    inverted: bool,
    running: bool,
    speed: usize,
    brush: f32,
    seed: u64,
    warmup: u64,
    texture: Option<egui::TextureHandle>,
    dirty: bool,
    status: String,
    save_error: bool,
    frame_ms: f32,
    last_brush: Option<egui::Pos2>,
}

impl Studio {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        configure_style(&cc.egui_ctx);
        Self {
            sim: Simulation::new(256, 42),
            params: PRESETS[0].2,
            preset: 0,
            palette: 0,
            contrast: 1.0,
            inverted: false,
            running: true,
            speed: 24,
            brush: 5.0,
            seed: 42,
            warmup: 1600,
            texture: None,
            dirty: true,
            status: String::new(),
            save_error: false,
            frame_ms: 0.0,
            last_brush: None,
        }
    }

    fn reset(&mut self, evolve: bool) {
        self.sim = Simulation::new(self.sim.size, self.seed);
        self.warmup = if evolve { 1600 } else { 0 };
        self.dirty = true;
        self.last_brush = None;
    }

    fn export(&mut self) {
        let result = (|| -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
            let executable = std::env::current_exe()?;
            let dir = executable
                .parent()
                .ok_or("実行ファイルのフォルダーを取得できません")?
                .join("captures");
            std::fs::create_dir_all(&dir)?;
            let stamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
            let path = dir.join(format!("turing-{stamp}.png"));
            let p = self.params;
            let metadata = format!(
                "Gray-Scott; F={}; k={}; Du={}; Dv={}; seed={}; steps={}; palette={}; contrast={}; inverted={}",
                p.feed,
                p.kill,
                p.diffusion_u,
                p.diffusion_v,
                self.seed,
                self.sim.steps,
                palette::PALETTES[self.palette].0,
                self.contrast,
                self.inverted
            );
            export::save_png(
                &path,
                self.sim.size,
                &palette::rgb(&self.sim.cells, self.palette, self.contrast, self.inverted),
                &metadata,
            )?;
            Ok(path)
        })();
        match result {
            Ok(path) => {
                self.status = format!("保存しました: {}", path.display());
                self.save_error = false;
            }
            Err(e) => {
                self.status = format!("保存できませんでした: {e}");
                self.save_error = true;
            }
        }
    }

    fn controls(&mut self, ui: &mut egui::Ui) {
        ui.add_space(8.0);
        section(ui, "01", "パターン");
        ui.add_space(8.0);
        ui.horizontal_wrapped(|ui| {
            for (i, (name, _, _)) in PRESETS.iter().enumerate() {
                if ui
                    .selectable_label(self.preset == i && self.params == PRESETS[i].2, *name)
                    .clicked()
                {
                    self.preset = i;
                    self.params = PRESETS[i].2;
                    self.reset(true);
                    self.running = true;
                }
            }
        });
        ui.add_space(6.0);
        ui.label(
            RichText::new(if self.params == PRESETS[self.preset].2 {
                PRESETS[self.preset].1
            } else {
                "カスタム設定 · 模様の変化を探してみましょう。"
            })
            .small()
            .color(MUTED),
        );
        ui.add_space(18.0);
        section(ui, "02", "反応と拡散");
        ui.add_space(8.0);
        parameter(
            ui,
            "供給率",
            "F",
            &mut self.params.feed,
            0.0..=0.1,
            "物質 U の補給量。模様の密度や形を変えます。",
        );
        parameter(
            ui,
            "除去率",
            "k",
            &mut self.params.kill,
            0.0..=0.1,
            "物質 V の除去量。F との組み合わせで模様が変わります。",
        );
        parameter(
            ui,
            "U の拡散",
            "Du",
            &mut self.params.diffusion_u,
            0.05..=1.0,
            "物質 U が隣のセルに広がる速さ。",
        );
        parameter(
            ui,
            "V の拡散",
            "Dv",
            &mut self.params.diffusion_v,
            0.01..=0.5,
            "物質 V が隣のセルに広がる速さ。",
        );
        ui.add_space(4.0);
        ui.label(
            RichText::new(
                "変更はすぐ反映されます。模様が消えたら、プリセットを選び直してください。",
            )
            .size(11.0)
            .color(MUTED),
        );
        ui.add_space(18.0);
        section(ui, "03", "色のアトリエ");
        ui.spacing_mut().slider_width = 135.0;
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            for i in 0..palette::PALETTES.len() {
                let (rect, response) =
                    ui.allocate_exact_size(Vec2::new(46.0, 30.0), egui::Sense::click());
                draw_gradient(ui, rect.shrink(3.0), i);
                if i == self.palette {
                    ui.painter().rect_stroke(
                        rect,
                        5.0,
                        egui::Stroke::new(1.5, ACCENT),
                        egui::StrokeKind::Inside,
                    );
                }
                if response.on_hover_text(palette::PALETTES[i].0).clicked() {
                    self.palette = i;
                    self.dirty = true;
                }
            }
        });
        ui.horizontal(|ui| {
            ui.label(RichText::new(palette::PALETTES[self.palette].0).color(ACCENT));
            if ui.checkbox(&mut self.inverted, "色を反転").changed() {
                self.dirty = true;
            }
        });
        ui.horizontal(|ui| {
            ui.label("色の強さ");
            if ui
                .add(egui::Slider::new(&mut self.contrast, 0.5..=2.0).fixed_decimals(2))
                .changed()
            {
                self.dirty = true;
            }
        });
        ui.add_space(18.0);
        section(ui, "04", "シミュレーション");
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.label("速度");
            ui.add(egui::Slider::new(&mut self.speed, 1..=100).suffix(" step"));
        });
        ui.label(
            RichText::new("1フレームの最大ステップ数 / 負荷に応じて調整")
                .size(10.0)
                .color(MUTED),
        );
        ui.horizontal(|ui| {
            ui.label("解像度");
            let mut size = self.sim.size;
            egui::ComboBox::from_id_salt("resolution")
                .selected_text(format!("{size} × {size}"))
                .width(142.0)
                .show_ui(ui, |ui| {
                    for n in [128, 256, 384, 512] {
                        ui.selectable_value(&mut size, n, format!("{n} × {n}"));
                    }
                })
                .response
                .on_hover_text("変更すると初期状態から再生成します。");
            if size != self.sim.size {
                self.sim = Simulation::new(size, self.seed);
                self.reset(true);
            }
        });
        ui.horizontal(|ui| {
            ui.label("ブラシ");
            ui.add(
                egui::Slider::new(&mut self.brush, 1.0..=24.0)
                    .suffix(" px")
                    .fixed_decimals(0),
            );
        });
        ui.horizontal(|ui| {
            if ui
                .button("初期状態へ")
                .on_hover_text("現在の設定・同じシードで、0 ステップからやり直します。 R キー")
                .clicked()
            {
                self.reset(false);
            }
            if ui
                .button("新しい種")
                .on_hover_text("シードを変えて模様を再生成します。")
                .clicked()
            {
                self.seed = self.seed.wrapping_add(1).max(1);
                self.reset(true);
            }
        });
        ui.add_space(8.0);
        egui::CollapsingHeader::new("この模様のしくみ").show(ui, |ui| {
            ui.label(RichText::new("2つの物質が反応し、異なる速さで拡散することで、自発的に模様が生まれます。Gray–Scott モデルによる反応拡散の可視化です。").small().color(MUTED));
            ui.label(RichText::new("色は物質 V の濃度を表します。上下・左右の端はつながっています。任意の設定で模様が残るとは限りません。").small().color(MUTED));
            ui.monospace("dU = Du ∇²U − UV² + F(1−U)");
            ui.monospace("dV = Dv ∇²V + UV² − (F+k)V");
            ui.label(RichText::new("Space: 再生 / 停止   R: 初期状態へ").small());
        });
    }
}

impl eframe::App for Studio {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if !ctx.wants_keyboard_input() {
            if ctx.input(|i| i.key_pressed(egui::Key::Space)) {
                self.running = !self.running;
            }
            if ctx.input(|i| i.key_pressed(egui::Key::R)) {
                self.reset(false);
            }
        }
        egui::TopBottomPanel::top("header")
            .frame(
                egui::Frame::new()
                    .fill(BG)
                    .inner_margin(egui::Margin::symmetric(24, 16)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label(
                            RichText::new("TURING STUDIO")
                                .size(23.0)
                                .strong()
                                .color(TEXT),
                        );
                        ui.label(
                            RichText::new("反応拡散のアトリエ  /  秩序は、ゆらぎから生まれる。")
                                .size(12.0)
                                .color(MUTED),
                        );
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .add_sized([112.0, 36.0], egui::Button::new("PNG を保存"))
                            .clicked()
                        {
                            self.export();
                        }
                        if ui
                            .add_enabled(
                                !self.running,
                                egui::Button::new("1 step").min_size(Vec2::new(64.0, 36.0)),
                            )
                            .clicked()
                        {
                            self.sim.step(self.params);
                            self.dirty = true;
                        }
                        let label = if self.running {
                            "Ⅱ  一時停止"
                        } else {
                            "▶  再生"
                        };
                        if ui
                            .add_sized(
                                [112.0, 36.0],
                                egui::Button::new(RichText::new(label).color(BG)).fill(ACCENT),
                            )
                            .clicked()
                        {
                            self.running = !self.running;
                        }
                    });
                });
            });
        egui::TopBottomPanel::bottom("footer")
            .frame(
                egui::Frame::new()
                    .fill(BG)
                    .inner_margin(egui::Margin::symmetric(24, 9)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(if self.running {
                            "●  LIVE"
                        } else {
                            "●  PAUSED"
                        })
                        .size(11.0)
                        .color(if self.running { ACCENT } else { MUTED }),
                    );
                    ui.separator();
                    ui.label(
                        RichText::new(format!(
                            "{} × {}    /    {:06} steps    /    seed {}",
                            self.sim.size, self.sim.size, self.sim.steps, self.seed
                        ))
                        .monospace()
                        .size(11.0)
                        .color(MUTED),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            RichText::new(format!("GRAY–SCOTT    ·    {:.1} ms", self.frame_ms))
                                .size(11.0)
                                .color(MUTED),
                        );
                    });
                });
            });
        egui::SidePanel::left("controls")
            .exact_width(310.0)
            .resizable(false)
            .frame(
                egui::Frame::new()
                    .fill(PANEL)
                    .inner_margin(egui::Margin::symmetric(20, 12)),
            )
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| self.controls(ui));
            });

        if self.running {
            let start = Instant::now();
            let count = if self.sim.steps < self.warmup {
                160
            } else {
                self.speed
            };
            for _ in 0..count {
                self.sim.step(self.params);
                if start.elapsed() > Duration::from_millis(12) {
                    break;
                }
            }
            self.frame_ms = start.elapsed().as_secs_f32() * 1000.0;
            self.dirty = true;
        }
        if self.dirty {
            let rgb = palette::rgb(&self.sim.cells, self.palette, self.contrast, self.inverted);
            let image = egui::ColorImage::from_rgb([self.sim.size, self.sim.size], &rgb);
            if let Some(texture) = &mut self.texture {
                texture.set(image, egui::TextureOptions::LINEAR);
            } else {
                self.texture = Some(ctx.load_texture(
                    "reaction-diffusion",
                    image,
                    egui::TextureOptions::LINEAR,
                ));
            }
            self.dirty = false;
        }
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(BG).inner_margin(24))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("PATTERN CANVAS")
                            .size(11.0)
                            .strong()
                            .color(MUTED),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            RichText::new(if self.sim.steps < self.warmup {
                                "模様を育てています…"
                            } else {
                                "ドラッグで種を描く · 右ドラッグで消す"
                            })
                            .size(11.0)
                            .color(MUTED),
                        );
                    });
                });
                ui.add_space(12.0);
                if !self.status.is_empty() {
                    ui.label(
                        RichText::new(&self.status)
                            .size(11.0)
                            .color(if self.save_error {
                                Color32::from_rgb(255, 155, 140)
                            } else {
                                ACCENT
                            }),
                    );
                    ui.add_space(6.0);
                }
                let available = ui.available_size();
                let side = available.x.min((available.y - 38.0).max(100.0));
                let (space, _) =
                    ui.allocate_exact_size(Vec2::new(available.x, side), egui::Sense::hover());
                let rect = egui::Rect::from_center_size(space.center(), Vec2::splat(side));
                let response =
                    ui.interact(rect, egui::Id::new("canvas"), egui::Sense::click_and_drag());
                if let Some(texture) = &self.texture {
                    ui.painter().image(
                        texture.id(),
                        rect,
                        egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
                        Color32::WHITE,
                    );
                    ui.painter().rect_stroke(
                        rect,
                        2.0,
                        egui::Stroke::new(1.0, Color32::from_rgb(54, 68, 80)),
                        egui::StrokeKind::Outside,
                    );
                }
                if response.hovered() || response.dragged() {
                    if let Some(pos) = response
                        .interact_pointer_pos()
                        .or_else(|| response.hover_pos())
                    {
                        let point = egui::pos2(
                            (pos.x - rect.left()) / side * self.sim.size as f32,
                            (pos.y - rect.top()) / side * self.sim.size as f32,
                        );
                        let down =
                            ctx.input(|i| i.pointer.primary_down() || i.pointer.secondary_down());
                        if down && rect.contains(pos) {
                            let erase = ctx.input(|i| i.pointer.secondary_down());
                            let from = self.last_brush.unwrap_or(point);
                            let segments = ((from.distance(point) / (self.brush * 0.5)).ceil()
                                as usize)
                                .clamp(1, 1024);
                            for s in 1..=segments {
                                let p = from.lerp(point, s as f32 / segments as f32);
                                self.sim.paint(p.x, p.y, self.brush, erase);
                            }
                            self.last_brush = Some(point);
                            self.dirty = true;
                        } else {
                            self.last_brush = None;
                        }
                        ui.painter().circle_stroke(
                            pos,
                            self.brush / self.sim.size as f32 * side,
                            egui::Stroke::new(1.0, Color32::from_white_alpha(170)),
                        );
                    }
                } else {
                    self.last_brush = None;
                }
                ui.add_space(14.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new("V 濃度  低").size(10.0).color(MUTED));
                    let (rect, _) =
                        ui.allocate_exact_size(Vec2::new(140.0, 7.0), egui::Sense::hover());
                    // Legend respects the same palette, inversion and intensity as the canvas.
                    for i in 0..140 {
                        let t = (i as f32 / 139.0).powf(1.0 / self.contrast);
                        let [r, g, b] =
                            palette::color(self.palette, if self.inverted { 1.0 - t } else { t });
                        let strip = egui::Rect::from_min_size(
                            rect.min + Vec2::new(i as f32, 0.0),
                            Vec2::new(1.0, rect.height()),
                        );
                        ui.painter()
                            .rect_filled(strip, 0.0, Color32::from_rgb(r, g, b));
                    }
                    ui.label(RichText::new("高").size(10.0).color(MUTED));
                });
            });
        if self.running || self.dirty {
            ctx.request_repaint_after(Duration::from_millis(16));
        }
    }
}

fn section(ui: &mut egui::Ui, number: &str, title: &str) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(number).monospace().size(11.0).color(ACCENT));
        ui.label(RichText::new(title).size(14.0).strong().color(TEXT));
    });
}

fn parameter(
    ui: &mut egui::Ui,
    title: &str,
    symbol: &str,
    value: &mut f32,
    range: std::ops::RangeInclusive<f32>,
    hint: &str,
) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(title).size(12.0))
            .on_hover_text(hint);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(RichText::new(symbol).monospace().size(11.0).color(MUTED));
        });
    });
    ui.add(
        egui::Slider::new(value, range)
            .fixed_decimals(4)
            .step_by(0.0001)
            .clamping(egui::SliderClamping::Always),
    );
    ui.add_space(5.0);
}

fn draw_gradient(ui: &egui::Ui, rect: egui::Rect, palette: usize) {
    for i in 0..32 {
        let [r, g, b] = palette::color(palette, i as f32 / 31.0);
        let strip = egui::Rect::from_min_max(
            egui::pos2(rect.left() + rect.width() * i as f32 / 32.0, rect.top()),
            egui::pos2(
                rect.left() + rect.width() * (i + 1) as f32 / 32.0,
                rect.bottom(),
            ),
        );
        ui.painter()
            .rect_filled(strip, 0.0, Color32::from_rgb(r, g, b));
    }
}

fn configure_style(ctx: &egui::Context) {
    ctx.set_theme(egui::Theme::Dark);
    let mut fonts = egui::FontDefinitions::default();
    // Use the OS font in place; no proprietary font is copied into the project.
    for path in [
        "C:/Windows/Fonts/YuGothM.ttc",
        "/System/Library/Fonts/ヒラギノ角ゴシック W3.ttc",
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
    ] {
        if let Ok(bytes) = std::fs::read(path) {
            fonts
                .font_data
                .insert("japanese".into(), egui::FontData::from_owned(bytes).into());
            for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
                fonts
                    .families
                    .entry(family)
                    .or_default()
                    .push("japanese".into());
            }
            break;
        }
    }
    ctx.set_fonts(fonts);
    let mut style = (*ctx.style()).clone();
    style.visuals = egui::Visuals::dark();
    style.visuals.panel_fill = BG;
    style.visuals.override_text_color = Some(TEXT);
    style.visuals.selection.bg_fill = Color32::from_rgb(40, 81, 78);
    style.visuals.selection.stroke = egui::Stroke::new(1.0, ACCENT);
    style.visuals.widgets.inactive.bg_fill = Color32::from_rgb(32, 43, 56);
    style.visuals.widgets.hovered.bg_fill = Color32::from_rgb(47, 66, 78);
    style.visuals.widgets.active.bg_fill = Color32::from_rgb(54, 93, 94);
    style.visuals.widgets.inactive.corner_radius = egui::CornerRadius::same(5);
    style.visuals.widgets.hovered.corner_radius = egui::CornerRadius::same(5);
    style.visuals.widgets.active.corner_radius = egui::CornerRadius::same(5);
    style.spacing.item_spacing = Vec2::new(9.0, 6.0);
    style.spacing.button_padding = Vec2::new(10.0, 5.0);
    style.spacing.slider_width = 178.0;
    style
        .text_styles
        .insert(egui::TextStyle::Body, egui::FontId::proportional(13.0));
    style
        .text_styles
        .insert(egui::TextStyle::Button, egui::FontId::proportional(12.0));
    ctx.set_style(style);
}
