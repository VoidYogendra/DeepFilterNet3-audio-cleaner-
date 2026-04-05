#![windows_subsystem = "windows"]
use eframe::egui;
use rfd::FileDialog;
use std::path::PathBuf;
use std::process::Command;
use std::sync::mpsc;
use std::thread;

use df::tract::{DfParams, DfTract, RuntimeParams};
use ndarray::Array2;

fn get_ffmpeg_command() -> String {
    #[cfg(target_os = "windows")]
    {
        if let Ok(mut exe_path) = std::env::current_exe() {
            exe_path.pop();
            let ffmpeg_exe = exe_path.join("ffmpeg.exe");
            if ffmpeg_exe.exists() {
                return ffmpeg_exe.to_string_lossy().into_owned();
            }
        }
        "ffmpeg.exe".to_string()
    }

    #[cfg(not(target_os = "windows"))]
    {
        "ffmpeg".to_string()
    }
}

fn main() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "DeepFilterNet3 Media Cleaner by Void Yogendra",
        options,
        Box::new(|cc| {
            egui_extras::install_image_loaders(&cc.egui_ctx);
            let screen_rect = cc.egui_ctx.input(|x| x.screen_rect());
            let screen_size = screen_rect.size();
            cc.egui_ctx
                .send_viewport_cmd(egui::ViewportCommand::InnerSize(screen_size * 0.5));
            Box::new(AudioCleanerApp::default())
        }),
    )
}

struct AudioCleanerApp {
    frames: Vec<egui::TextureHandle>,
    frame_index: usize,
    last_frame_time: f64,
    show_welcome_screen: bool,
    selected_file: Option<PathBuf>,
    is_processing: bool,
    status_message: String,
    receiver: Option<mpsc::Receiver<String>>,
    volume_boost: f32,
}

impl Default for AudioCleanerApp {
    fn default() -> Self {
        let ffmpeg_available = Command::new(get_ffmpeg_command())
            .arg("-version")
            .output()
            .is_ok();

        let status = if ffmpeg_available {
            "Ready.".to_string()
        } else {
            "Error: FFmpeg not found! Please install it.".to_string()
        };
        Self {
            frames: Vec::new(),
            frame_index: 0,
            last_frame_time: 0.0,
            show_welcome_screen: true,
            selected_file: None,
            is_processing: false,
            status_message: status,
            receiver: None,
            volume_boost: 1.0,
        }
    }
}

impl eframe::App for AudioCleanerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.frames.is_empty() {
            let base_path = if cfg!(target_os = "windows") {
                std::env::current_exe()
                    .ok()
                    .and_then(|p| p.parent().map(|parent| parent.to_path_buf()))
                    .unwrap_or_else(|| PathBuf::from("."))
            } else {
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            };

            for i in 1..=227 {
                let path = base_path.join("assets").join(format!("frame_{:03}.png", i));

                if let Ok(img) = image::open(&path) {
                    let img = img.to_rgba8();
                    let size = [img.width() as _, img.height() as _];
                    let pixels = img.into_raw();

                    let tex = ctx.load_texture(
                        format!("frame_{}", i),
                        egui::ColorImage::from_rgba_unmultiplied(size, &pixels),
                        Default::default(),
                    );
                    self.frames.push(tex);
                } else {
                    if i == 1 {
                        self.status_message = format!("Failed to find assets at: {:?}", path);
                    }
                    break;
                }
            }
        }

        if self.show_welcome_screen {
            egui::CentralPanel::default().show(ctx, |ui| {
                let available = ui.available_size();

                ui.allocate_ui_with_layout(
                    available,
                    egui::Layout::top_down(egui::Align::Center),
                    |ui| {
                        let content_height = 300.0;
                        let top_space = (available.y - content_height) * 0.5;
                        if top_space > 0.0 {
                            ui.add_space(top_space);
                        }

                        ui.heading("Welcome to the DFN3 Audio Cleaner");
                        ui.add_space(10.0);

                        let time = ctx.input(|i| i.time);

                        if !self.frames.is_empty() && time - self.last_frame_time > 0.03 {
                            self.frame_index = (self.frame_index + 1) % self.frames.len();
                            self.last_frame_time = time;
                        }

                        if let Some(tex) = self.frames.get(self.frame_index) {
                            ui.add(
                                egui::Image::new(tex)
                                    .rounding(15.0)
                                    .fit_to_exact_size(egui::vec2(300.0, 300.0)),
                            );
                        } else {
                            ui.label("Loading animation...");
                        }

                        ui.add_space(20.0);

                        if ui
                            .add_sized([100.0, 50.0], egui::Button::new("Enter App"))
                            .clicked()
                        {
                            self.show_welcome_screen = false;
                        }
                    },
                );
            });

            ctx.request_repaint_after(std::time::Duration::from_millis(16));
            return;
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            let available = ui.available_size();

            ui.allocate_ui_with_layout(
                available,
                egui::Layout::top_down(egui::Align::Center),
                |ui| {
                    ui.heading("DeepFilter Media Cleaner");
                    ui.add_space(20.0);
                    if ui
                        .button("Select Media File (WAV, MP3, MP4, MKV...)")
                        .clicked()
                    {
                        if let Some(path) = FileDialog::new()
                            .add_filter("Media", &["wav", "mp3", "mp4", "mkv", "mov", "avi", "flv"])
                            .pick_file()
                        {
                            self.selected_file = Some(path);
                            self.status_message = "File selected.".to_string();
                        }
                    }

                    if let Some(path) = &self.selected_file {
                        ui.label(format!("Selected: {}", path.display()));
                    }

                    ui.add_space(20.0);

                    ui.vertical_centered(|ui| {
                        ui.label("Output Gain");

                        ui.add_sized(
                            [250.0, 20.0],
                            egui::Slider::new(&mut self.volume_boost, 1.0..=5.0)
                                .text("x")
                                .step_by(0.1),
                        );
                    });

                    ui.add_space(20.0);

                    ui.add_enabled_ui(self.selected_file.is_some() && !self.is_processing, |ui| {
                        if ui.button("Clean Audio").clicked() {
                            self.is_processing = true;
                            self.status_message = "Processing...".to_string();

                            let (tx, rx) = mpsc::channel();
                            self.receiver = Some(rx);
                            let file_path = self.selected_file.clone().unwrap();

                            let current_boost = self.volume_boost;

                            thread::spawn(move || {
                                let result: Result<(), Box<dyn std::error::Error + Send + Sync>> =
                                    (|| {
                                        tx.send("Preparing temporary files...".to_string())?;
                                        let temp_dir = tempfile::tempdir()?;
                                        let temp_in_wav = temp_dir.path().join("in.wav");
                                        let temp_out_wav = temp_dir.path().join("out.wav");

                                        tx.send(
                                            "Extracting audio to 48kHz WAV via FFmpeg..."
                                                .to_string(),
                                        )?;
                                        let extract_status = Command::new("ffmpeg")
                                            .args([
                                                "-y",
                                                "-i",
                                                file_path.to_str().unwrap(),
                                                "-ar",
                                                "48000",
                                                temp_in_wav.to_str().unwrap(),
                                            ])
                                            .output()?;

                                        if !extract_status.status.success() {
                                            return Err(
                                                "FFmpeg extraction failed. Is FFmpeg installed?"
                                                    .into(),
                                            );
                                        }

                                        tx.send("Loading DeepFilterNet model...".to_string())?;

                                        let mut reader = hound::WavReader::open(&temp_in_wav)?;
                                        let spec = reader.spec();

                                        let samples: Vec<f32> = match spec.sample_format {
                                            hound::SampleFormat::Int => reader
                                                .samples::<i16>()
                                                .map(|s| s.unwrap_or(0) as f32 / i16::MAX as f32)
                                                .collect(),
                                            hound::SampleFormat::Float => reader
                                                .samples::<f32>()
                                                .map(|s| s.unwrap_or(0.0))
                                                .collect(),
                                        };

                                        let channels = spec.channels as usize;
                                        let num_samples = samples.len() / channels;

                                        let mut deinterleaved = vec![0.0f32; samples.len()];
                                        for (i, &sample) in samples.iter().enumerate() {
                                            deinterleaved
                                                [(i % channels) * num_samples + (i / channels)] =
                                                sample;
                                        }

                                        let noisy_array = Array2::from_shape_vec(
                                            (channels, num_samples),
                                            deinterleaved,
                                        )?;
                                        let mut enh_array =
                                            Array2::<f32>::zeros((channels, num_samples));

                                        let hop_size = 480;

                                        for ch in 0..channels {
                                            tx.send(format!(
                                                "Applying AI Noise Reduction (Channel {} of {})...",
                                                ch + 1,
                                                channels
                                            ))?;
                                            let channel_in =
                                                noisy_array.slice(ndarray::s![ch..ch + 1, ..]);
                                            let mut channel_out =
                                                enh_array.slice_mut(ndarray::s![ch..ch + 1, ..]);
                                            let mut df_tract = DfTract::new(
                                                DfParams::default(),
                                                &RuntimeParams::default(),
                                            )?;

                                            for (in_chunk, mut out_chunk) in channel_in
                                                .axis_chunks_iter(ndarray::Axis(1), hop_size)
                                                .zip(channel_out.axis_chunks_iter_mut(
                                                    ndarray::Axis(1),
                                                    hop_size,
                                                ))
                                            {
                                                let current_chunk_len = in_chunk.shape()[1];
                                                if current_chunk_len == hop_size {
                                                    df_tract.process(in_chunk, out_chunk)?;
                                                } else {
                                                    let mut padded_in =
                                                        Array2::<f32>::zeros((1, hop_size));
                                                    let mut padded_out =
                                                        Array2::<f32>::zeros((1, hop_size));
                                                    padded_in
                                                        .slice_mut(ndarray::s![
                                                            ..,
                                                            ..current_chunk_len
                                                        ])
                                                        .assign(&in_chunk);
                                                    df_tract.process(
                                                        padded_in.view(),
                                                        padded_out.view_mut(),
                                                    )?;
                                                    out_chunk.assign(&padded_out.slice(
                                                        ndarray::s![.., ..current_chunk_len],
                                                    ));
                                                }
                                            }
                                        }

                                        tx.send("Writing cleaned temporary WAV...".to_string())?;
                                        let out_spec = hound::WavSpec {
                                            channels: spec.channels,
                                            sample_rate: spec.sample_rate,
                                            bits_per_sample: 16,
                                            sample_format: hound::SampleFormat::Int,
                                        };
                                        let mut writer =
                                            hound::WavWriter::create(&temp_out_wav, out_spec)?;

                                        for s_idx in 0..num_samples {
                                            for ch in 0..channels {
                                                let sample = enh_array[[ch, s_idx]];
                                                let out_sample = (sample * i16::MAX as f32)
                                                    .clamp(i16::MIN as f32, i16::MAX as f32)
                                                    as i16;
                                                writer.write_sample(out_sample)?;
                                            }
                                        }
                                        writer.finalize()?;

                                        tx.send(
                                            "Applying volume and merging formats...".to_string(),
                                        )?;

                                        let ext = file_path
                                            .extension()
                                            .unwrap_or_default()
                                            .to_string_lossy()
                                            .to_lowercase();
                                        let output_path = file_path.with_file_name(format!(
                                            "{}_cleaned.{}",
                                            file_path.file_stem().unwrap().to_string_lossy(),
                                            ext
                                        ));

                                        let mut cmd = Command::new("ffmpeg");
                                        cmd.arg("-y");

                                        let vol_filter = format!(
                                            "volume={:.2},alimiter=limit=0.95:attack=5:release=50",
                                            current_boost
                                        );

                                        if ext == "wav" {
                                            cmd.args([
                                                "-i",
                                                temp_out_wav.to_str().unwrap(),
                                                "-filter:a",
                                                &vol_filter,
                                                output_path.to_str().unwrap(),
                                            ]);
                                            if !cmd.output()?.status.success() {
                                                return Err("WAV volume processing failed".into());
                                            }
                                        } else if ext == "mp3" {
                                            cmd.args([
                                                "-i",
                                                temp_out_wav.to_str().unwrap(),
                                                "-filter:a",
                                                &vol_filter,
                                                "-c:a",
                                                "libmp3lame",
                                                "-q:a",
                                                "2",
                                                output_path.to_str().unwrap(),
                                            ]);
                                            if !cmd.output()?.status.success() {
                                                return Err("MP3 encoding failed".into());
                                            }
                                        } else {
                                            cmd.args([
                                                "-i",
                                                file_path.to_str().unwrap(),
                                                "-i",
                                                temp_out_wav.to_str().unwrap(),
                                                "-map",
                                                "0:v?",
                                                "-map",
                                                "1:a",
                                                "-c:v",
                                                "copy",
                                                "-c:a",
                                                "aac",
                                                "-b:a",
                                                "256k",
                                                "-filter:a",
                                                &vol_filter,
                                                output_path.to_str().unwrap(),
                                            ]);
                                            if !cmd.output()?.status.success() {
                                                return Err("Video merging failed".into());
                                            }
                                        }

                                        tx.send(format!(
                                            "Complete! Saved as {}",
                                            output_path.file_name().unwrap().to_string_lossy()
                                        ))?;

                                        Ok(())
                                    })();

                                if let Err(e) = result {
                                    let _ = tx.send(format!("Error: {}", e));
                                }
                            });
                        }
                    });

                    ui.add_space(20.0);

                    if let Some(rx) = &self.receiver {
                        if let Ok(msg) = rx.try_recv() {
                            self.status_message = msg.clone();
                            if msg.contains("Complete") || msg.contains("Error") {
                                self.is_processing = false;
                            }
                        }
                    }

                    ui.label(egui::RichText::new(&self.status_message));

                    if self.is_processing {
                        ctx.request_repaint();
                    }
                },
            );
        });
    }
}
