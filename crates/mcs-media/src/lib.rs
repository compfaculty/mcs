//! Bounded local image/PDF adapters. Terminal rendering belongs to mcs-tui.
pub mod process;
pub mod video;
pub use image::{self, DynamicImage};
use std::{
    fs::{self, File, OpenOptions},
    io::{self, BufReader, Cursor, Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, SystemTime},
};

pub const MAX_PIXELS: u64 = 16 * 1024 * 1024;
pub const MAX_ENCODED: u64 = 64 * 1024 * 1024;
const MAX_PDF: u64 = 128 * 1024 * 1024;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Text,
    Image,
    Pdf,
    Video,
}

pub fn open_regular(path: &Path) -> io::Result<File> {
    if !fs::metadata(path)?.is_file() {
        return Err(io::Error::other("Only local regular files can be viewed"));
    }
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NONBLOCK);
    }
    let file = options.open(path)?;
    if !file.metadata()?.is_file() {
        return Err(io::Error::other("File is no longer regular"));
    }
    Ok(file)
}
pub fn classify(path: &Path) -> io::Result<Kind> {
    let mut file = open_regular(path)?;
    let mut header = [0; 16];
    let count = file.read(&mut header)?;
    let header = &header[..count];
    if header.starts_with(b"\x89PNG\r\n\x1a\n") || header.starts_with(b"\xff\xd8\xff") {
        return Ok(Kind::Image);
    }
    if header.starts_with(b"%PDF-") {
        return Ok(Kind::Pdf);
    }
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match extension.as_str() {
        "png" | "jpg" | "jpeg" => Ok(Kind::Image),
        "pdf" => Ok(Kind::Pdf),
        "mp4" | "mkv" | "mov" | "webm" | "m4v" | "avi" => Ok(Kind::Video),
        "m3u" | "m3u8" | "pls" | "gif" | "svg" | "heic" => Err(io::Error::other(
            "This file type is not supported by the viewer",
        )),
        _ => Ok(Kind::Text),
    }
}
fn image_error(error: impl std::fmt::Display) -> io::Error {
    io::Error::other(error.to_string())
}
pub fn decode(reader: impl io::BufRead + io::Seek) -> io::Result<DynamicImage> {
    let reader = image::ImageReader::new(reader)
        .with_guessed_format()
        .map_err(image_error)?;
    if !matches!(
        reader.format(),
        Some(image::ImageFormat::Png | image::ImageFormat::Jpeg)
    ) {
        return Err(io::Error::other("Only PNG/JPEG images are enabled"));
    }
    decode_limited(reader)
}

// PNM is enabled only for the bounded internal Poppler pipe. Public image inputs
// still pass the PNG/JPEG allowlist above. Never decompress PDF pages to PNG only
// to immediately decode them again.
fn decode_pdf(bytes: Vec<u8>) -> io::Result<DynamicImage> {
    if !bytes.starts_with(b"P6") {
        return Err(io::Error::other("Poppler did not return binary RGB pixels"));
    }
    decode_limited(image::ImageReader::with_format(
        Cursor::new(bytes),
        image::ImageFormat::Pnm,
    ))
}
fn decode_limited(
    mut reader: image::ImageReader<impl io::BufRead + io::Seek>,
) -> io::Result<DynamicImage> {
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(16_384);
    limits.max_image_height = Some(16_384);
    limits.max_alloc = Some(64 * 1024 * 1024);
    reader.limits(limits);
    let decoder = reader.into_decoder().map_err(image_error)?;
    use image::ImageDecoder;
    let (width, height) = decoder.dimensions();
    if u64::from(width) * u64::from(height) > MAX_PIXELS || decoder.total_bytes() > 64 * 1024 * 1024
    {
        return Err(io::Error::other(
            "Image exceeds 16 megapixels / 64 MiB decoded limit",
        ));
    }
    DynamicImage::from_decoder(decoder).map_err(image_error)
}
pub fn load_image(path: &Path) -> io::Result<DynamicImage> {
    let file = open_regular(path)?;
    if file.metadata()?.len() > MAX_ENCODED {
        return Err(io::Error::other("Image exceeds 64 MiB encoded limit"));
    }
    let len = file.metadata()?.len().min(MAX_ENCODED);
    decode(BufReader::new(BoundedFile {
        file,
        position: 0,
        len,
    }))
}

// Preserve Seek for the decoder while bounding encoded reads if the file grows after stat.
struct BoundedFile {
    file: File,
    position: u64,
    len: u64,
}
impl Read for BoundedFile {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let count = buffer
            .len()
            .min(self.len.saturating_sub(self.position) as usize);
        let read = self.file.read(&mut buffer[..count])?;
        self.position += read as u64;
        Ok(read)
    }
}
impl Seek for BoundedFile {
    fn seek(&mut self, from: SeekFrom) -> io::Result<u64> {
        let target = match from {
            SeekFrom::Start(value) => i128::from(value),
            SeekFrom::Current(value) => i128::from(self.position) + i128::from(value),
            SeekFrom::End(value) => i128::from(self.len) + i128::from(value),
        };
        if target < 0 || target > i128::from(self.len) {
            return Err(io::Error::other("Image seek outside bounded input"));
        }
        self.position = self.file.seek(SeekFrom::Start(target as u64))?;
        Ok(self.position)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ImageView {
    pub width: u32,
    pub height: u32,
    pub zoom: i8,
    pub pan_x: u32,
    pub pan_y: u32,
}
/// The raster budget is independent of the terminal's physical placement size.
pub struct ViewportImage {
    pub image: DynamicImage,
    pub display_width: u32,
    pub display_height: u32,
}

const MAX_VIEW_PIXELS: u64 = 4 * 1024 * 1024;
fn raster_size(width: u32, height: u32) -> (u32, u32) {
    let width = width.max(1);
    let height = height.max(1);
    let scale = 1.0f64
        .min(4096.0 / f64::from(width))
        .min(4096.0 / f64::from(height))
        .min((MAX_VIEW_PIXELS as f64 / (f64::from(width) * f64::from(height))).sqrt());
    let mut out_width = (f64::from(width) * scale).floor().max(1.0) as u32;
    let out_height = (f64::from(height) * scale).floor().max(1.0) as u32;
    // Enforce the exact integer budget even at floating-point rounding boundaries.
    out_width = out_width
        .min((MAX_VIEW_PIXELS / u64::from(out_height)) as u32)
        .max(1);
    (out_width, out_height)
}

/// Crop before resizing, so zoom never materializes a giant intermediate bitmap.
/// Oversized Retina windows downsample the encoded raster, not the displayed cell rectangle.
pub fn render_viewport(source: &DynamicImage, view: ImageView) -> io::Result<ViewportImage> {
    if source.width() == 0 || source.height() == 0 {
        return Err(io::Error::other("Image dimensions must be nonzero"));
    }
    let width = view.width.max(1);
    let height = view.height.max(1);
    let fit = (f64::from(width) / f64::from(source.width()))
        .min(f64::from(height) / f64::from(source.height()));
    let scale = fit * 2f64.powf(f64::from(view.zoom.clamp(-4, 6)) / 2.0);
    let crop_width = (f64::from(width) / scale)
        .ceil()
        .clamp(1.0, f64::from(source.width())) as u32;
    let crop_height = (f64::from(height) / scale)
        .ceil()
        .clamp(1.0, f64::from(source.height())) as u32;
    let x = view.pan_x.min(source.width() - crop_width);
    let y = view.pan_y.min(source.height() - crop_height);
    let crop = image::imageops::crop_imm(source, x, y, crop_width, crop_height);
    let ratio =
        (f64::from(width) / f64::from(crop_width)).min(f64::from(height) / f64::from(crop_height));
    let display_width = (f64::from(crop_width) * ratio).floor().max(1.0) as u32;
    let display_height = (f64::from(crop_height) * ratio).floor().max(1.0) as u32;
    // Kitty scales the virtual placement. Upsampling here adds CPU/bytes, not detail.
    let (out_width, out_height) = raster_size(
        display_width.min(crop_width),
        display_height.min(crop_height),
    );
    let image = if (out_width, out_height) == (crop_width, crop_height) {
        source.crop_imm(x, y, crop_width, crop_height)
    } else {
        DynamicImage::ImageRgba8(image::imageops::resize(
            &*crop,
            out_width,
            out_height,
            image::imageops::FilterType::Triangle,
        ))
    };
    Ok(ViewportImage {
        image,
        display_width,
        display_height,
    })
}

pub fn viewport(source: &DynamicImage, view: ImageView) -> io::Result<DynamicImage> {
    render_viewport(source, view).map(|rendered| rendered.image)
}

#[derive(Clone)]
pub struct PdfTools {
    pub info: PathBuf,
    pub render: PathBuf,
    pub text: PathBuf,
}
impl Default for PdfTools {
    fn default() -> Self {
        Self {
            info: "pdfinfo".into(),
            render: "pdftoppm".into(),
            text: "pdftotext".into(),
        }
    }
}
pub struct PdfSearch<'a> {
    pub query: &'a str,
    pub current: u32,
    pub reverse: bool,
    pub repeat: bool,
}
pub struct Pdf {
    path: PathBuf,
    tools: PdfTools,
    pub pages: u32,
    len: u64,
    modified: Option<SystemTime>,
}
fn helper(
    name: &Path,
    args: Vec<std::ffi::OsString>,
    limit: usize,
    token: &AtomicU64,
    generation: u64,
) -> io::Result<Vec<u8>> {
    process::capture(
        name.as_os_str(),
        &args,
        limit,
        Duration::from_secs(10),
        token,
        generation,
    )
}
impl Pdf {
    pub fn open(path: &Path, token: &AtomicU64, generation: u64) -> io::Result<Self> {
        Self::open_with(path, PdfTools::default(), token, generation)
    }
    pub fn open_with(
        path: &Path,
        tools: PdfTools,
        token: &AtomicU64,
        generation: u64,
    ) -> io::Result<Self> {
        let file = open_regular(path)?;
        let metadata = file.metadata()?;
        if metadata.len() > MAX_PDF {
            return Err(io::Error::other("PDF exceeds 128 MiB input limit"));
        }
        let path = fs::canonicalize(path)?;
        let info = helper(
            &tools.info,
            vec![path.as_os_str().into()],
            64 * 1024,
            token,
            generation,
        )?;
        let text = String::from_utf8_lossy(&info);
        let pages = text
            .lines()
            .find_map(|line| {
                line.strip_prefix("Pages:")
                    .and_then(|value| value.trim().parse::<u32>().ok())
            })
            .filter(|&pages| pages > 0 && pages <= 100_000)
            .ok_or_else(|| io::Error::other("PDF page count unavailable or exceeds 100,000"))?;
        Ok(Self {
            path,
            tools,
            pages,
            len: metadata.len(),
            modified: metadata.modified().ok(),
        })
    }
    fn validate(&self, page: u32) -> io::Result<()> {
        let metadata = fs::metadata(&self.path)?;
        if metadata.len() != self.len || metadata.modified().ok() != self.modified {
            return Err(io::Error::other("PDF changed; reload with r"));
        }
        if page == 0 || page > self.pages {
            return Err(io::Error::other("PDF page outside document"));
        }
        Ok(())
    }
    pub fn render(
        &self,
        page: u32,
        token: &AtomicU64,
        generation: u64,
    ) -> io::Result<DynamicImage> {
        self.validate(page)?;
        let bytes = helper(
            &self.tools.render,
            vec![
                "-f".into(),
                page.to_string().into(),
                "-l".into(),
                page.to_string().into(),
                "-singlefile".into(),
                "-scale-to".into(),
                "2048".into(),
                self.path.as_os_str().into(),
            ],
            MAX_ENCODED as usize,
            token,
            generation,
        )?;
        self.validate(page)?;
        decode_pdf(bytes)
    }
    pub fn search(
        &self,
        request: PdfSearch<'_>,
        token: &AtomicU64,
        generation: u64,
        mut progress: impl FnMut(u32),
    ) -> io::Result<Option<(u32, String)>> {
        let PdfSearch {
            query,
            current,
            reverse,
            repeat,
        } = request;
        if query.is_empty() || query.chars().count() > 256 {
            return Err(io::Error::other("Search query must be 1..256 characters"));
        }
        for step in 0..self.pages {
            if token.load(Ordering::Relaxed) != generation {
                return Err(io::Error::other("Cancelled"));
            }
            let distance = step + u32::from(repeat);
            let page = if reverse {
                (current - 1 + self.pages - distance % self.pages) % self.pages + 1
            } else {
                (current - 1 + distance) % self.pages + 1
            };
            self.validate(page)?;
            progress(page);
            let bytes = helper(
                &self.tools.text,
                vec![
                    "-f".into(),
                    page.to_string().into(),
                    "-l".into(),
                    page.to_string().into(),
                    "-enc".into(),
                    "UTF-8".into(),
                    self.path.as_os_str().into(),
                    "-".into(),
                ],
                1024 * 1024,
                token,
                generation,
            )?;
            let text = String::from_utf8_lossy(&bytes);
            if let Some(index) = text.find(query) {
                let snippet: String = text[index..].chars().take(180).collect();
                return Ok(Some((page, snippet)));
            }
        }
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::ImageFormat;
    #[test]
    fn image_decode_fit_zoom_pan_and_limits() {
        let source = DynamicImage::new_rgb8(100, 50);
        let mut bytes = Cursor::new(Vec::new());
        source.write_to(&mut bytes, ImageFormat::Png).unwrap();
        bytes.set_position(0);
        let decoded = decode(bytes).unwrap();
        assert_eq!(decoded.width(), 100);
        let view = ImageView {
            width: 40,
            height: 40,
            zoom: 0,
            pan_x: 0,
            pan_y: 0,
        };
        assert_eq!(viewport(&decoded, view).unwrap().height(), 20);
        let zoom = viewport(
            &decoded,
            ImageView {
                zoom: 4,
                pan_x: u32::MAX,
                pan_y: u32::MAX,
                ..view
            },
        )
        .unwrap();
        assert!(zoom.width() <= 40 && zoom.height() <= 40);
        let large = render_viewport(
            &decoded,
            ImageView {
                width: 4096,
                height: 4096,
                ..view
            },
        )
        .unwrap();
        assert_eq!((large.display_width, large.display_height), (4096, 2048));
        assert!(
            u64::from(large.image.width()) * u64::from(large.image.height()) <= MAX_VIEW_PIXELS
        );
        assert!(
            (f64::from(large.image.width()) / f64::from(large.image.height()) - 2.0).abs() < 0.01
        );
        assert!(decode(Cursor::new(b"not a png")).is_err());
        // Header advertises oversized dimensions; decoder rejects it before image allocation.
        let mut png = Cursor::new(Vec::new());
        source.write_to(&mut png, ImageFormat::Png).unwrap();
        let mut png = png.into_inner();
        png[16..20].copy_from_slice(&16_384u32.to_be_bytes());
        png[20..24].copy_from_slice(&16_384u32.to_be_bytes());
        let crc = crc32fast::hash(&png[12..29]);
        png[29..33].copy_from_slice(&crc.to_be_bytes());
        assert!(decode(Cursor::new(png))
            .unwrap_err()
            .to_string()
            .contains("16 megapixels"));
    }
    #[test]
    fn native_pixels_are_not_upscaled_or_exposed_as_public_pnm_support() {
        let source = DynamicImage::new_rgb8(100, 50);
        let rendered = render_viewport(
            &source,
            ImageView {
                width: 5000,
                height: 3000,
                zoom: 0,
                pan_x: 0,
                pan_y: 0,
            },
        )
        .unwrap();
        assert_eq!(
            (rendered.display_width, rendered.display_height),
            (5000, 2500)
        );
        assert_eq!((rendered.image.width(), rendered.image.height()), (100, 50));
        let bytes = b"P6\n1 1\n255\n\xff\0\0".to_vec();
        assert_eq!(decode_pdf(bytes.clone()).unwrap().width(), 1);
        assert!(decode(Cursor::new(bytes)).is_err());
        assert!(decode_pdf(b"P3\n1 1\n255\n255 0 0".to_vec()).is_err());
    }
    #[test]
    fn retina_ultrawide_and_extreme_viewports_keep_raster_budget() {
        for (width, height) in [
            (5120, 2880),
            (6016, 3384),
            (10000, 600),
            (600, 10000),
            (u32::MAX, u32::MAX),
            (0, 0),
            (u32::MAX, 1),
            (1, u32::MAX),
        ] {
            let (w, h) = raster_size(width, height);
            assert!((1..=4096).contains(&w) && (1..=4096).contains(&h));
            assert!(u64::from(w) * u64::from(h) <= MAX_VIEW_PIXELS);
        }
        assert_eq!(raster_size(640, 480), (640, 480));
        let (w, h) = raster_size(5120, 2880);
        assert!((f64::from(w) / f64::from(h) - 16.0 / 9.0).abs() < 0.01);
    }
    #[test]
    fn file_classification_rejects_special_files_and_playlists() {
        let dir = tempfile::tempdir().unwrap().keep();
        let path = dir.join("--evil $(touch nope).m3u");
        std::fs::write(&path, b"#EXTM3U\nhttp://example.invalid").unwrap();
        assert!(classify(&path).is_err());
        assert!(classify(&dir).is_err());
        let png = dir.join("wrong.txt");
        std::fs::write(&png, b"\x89PNG\r\n\x1a\n").unwrap();
        assert_eq!(classify(&png).unwrap(), Kind::Image);
    }
    #[cfg(unix)]
    mod unix {
        use super::*;
        use std::{ffi::OsString, os::unix::fs::PermissionsExt};
        fn script(dir: &Path, name: &str, body: &str) -> PathBuf {
            let path = dir.join(name);
            fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
            path
        }
        #[test]
        fn helpers_enforce_timeout_output_failure_and_cancel() {
            let dir = tempfile::tempdir().unwrap().keep();
            let token = AtomicU64::new(1);
            let slow = script(&dir, "slow", "exec /bin/sleep 5");
            let start = std::time::Instant::now();
            assert!(process::capture(
                slow.as_os_str(),
                &[],
                100,
                Duration::from_millis(30),
                &token,
                1
            )
            .unwrap_err()
            .to_string()
            .contains("timed out"));
            assert!(start.elapsed() < Duration::from_secs(2));
            let big = script(&dir, "big", "printf '12345678901234567890'");
            assert!(
                process::capture(big.as_os_str(), &[], 5, Duration::from_secs(1), &token, 1)
                    .is_err()
            );
            let fail = script(&dir, "fail", "printf 'bad PDF' >&2; exit 1");
            assert!(process::capture(
                fail.as_os_str(),
                &[],
                100,
                Duration::from_secs(1),
                &token,
                1
            )
            .unwrap_err()
            .to_string()
            .contains("bad PDF"));
            assert!(process::capture(
                slow.as_os_str(),
                &[],
                100,
                Duration::from_secs(1),
                &token,
                2
            )
            .unwrap_err()
            .to_string()
            .contains("Cancelled"));
        }
        #[test]
        fn pdf_page_navigation_search_and_revalidation_with_fake_tools() {
            let dir = tempfile::tempdir().unwrap().keep();
            let source = dir.join("document ; no shell.pdf");
            fs::write(&source, b"%PDF-fake").unwrap();
            let mut pixels = b"P6\n8 8\n255\n".to_vec();
            pixels.extend([0; 8 * 8 * 3]);
            fs::write(dir.join("render.ppm"), pixels).unwrap();
            let tools = PdfTools {
                info: script(&dir, "info", "printf 'Pages: 3\\n'"),
                render: script(&dir, "render", "cat \"$0.ppm\""),
                text: script(&dir, "text", "printf 'page %s needle' \"$2\""),
            };
            let token = AtomicU64::new(1);
            let pdf = Pdf::open_with(&source, tools, &token, 1).unwrap();
            assert_eq!(pdf.pages, 3);
            assert_eq!(pdf.render(2, &token, 1).unwrap().width(), 8);
            assert!(pdf.render(4, &token, 1).is_err());
            let found = pdf
                .search(
                    PdfSearch {
                        query: "needle",
                        current: 3,
                        reverse: false,
                        repeat: true,
                    },
                    &token,
                    1,
                    |_| {},
                )
                .unwrap()
                .unwrap();
            assert_eq!(found.0, 1);
            assert!(found.1.contains("needle"));
            assert!(pdf
                .search(
                    PdfSearch {
                        query: "absent",
                        current: 1,
                        reverse: true,
                        repeat: false
                    },
                    &token,
                    1,
                    |_| {}
                )
                .unwrap()
                .is_none());
            fs::OpenOptions::new()
                .append(true)
                .open(&source)
                .unwrap()
                .write_all(b"changed")
                .unwrap();
            assert!(pdf.render(1, &token, 1).is_err());
        }
        use std::io::Write;
        #[test]
        fn video_probe_accepts_slow_startup_and_remains_cancellable() {
            let dir = tempfile::tempdir().unwrap().keep();
            let source = dir.join("slow.mp4");
            fs::write(&source, b"fake").unwrap();
            let mpv = script(
                &dir,
                "mpv",
                "/bin/sleep 3.2; printf ' kitty terminal output\\n'",
            );
            let token = AtomicU64::new(1);
            assert!(video::Playback::prepare_with(mpv.as_os_str(), &source, &token, 1).is_ok());

            // A longer startup allowance must not delay cancellation of a stuck player.
            script(&dir, "mpv", "exec /bin/sleep 30");
            let start = std::time::Instant::now();
            std::thread::scope(|scope| {
                scope.spawn(|| {
                    std::thread::sleep(Duration::from_millis(100));
                    token.store(2, Ordering::Relaxed);
                });
                let error = video::Playback::prepare_with(mpv.as_os_str(), &source, &token, 1)
                    .err()
                    .expect("cancelled probe");
                assert!(error
                    .to_string()
                    .contains("mpv output-driver probe failed: Cancelled"));
            });
            assert!(start.elapsed() < Duration::from_secs(2));
        }
        #[test]
        fn video_handoff_disables_writes_and_handles_child_failures() {
            let dir = tempfile::tempdir().unwrap().keep();
            let source = dir.join("--input-conf=evil ; $(touch NO).mp4");
            fs::write(&source, b"fake").unwrap();
            let mpv = script(&dir, "mpv", "if [ \"$2\" = '--vo=help' ]; then printf ' kitty terminal output\\n'; else exit 0; fi");
            let token = AtomicU64::new(1);
            let playback =
                video::Playback::prepare_with(mpv.as_os_str(), &source, &token, 1).unwrap();
            assert_eq!(
                playback.args.last(),
                Some(&source.canonicalize().unwrap().into_os_string())
            );
            for arg in [
                "--input-default-bindings=no",
                "--input-builtin-bindings=no",
                "--save-position-on-quit=no",
                "--load-scripts=no",
                "--demuxer-lavf-o=protocol_whitelist=file",
            ] {
                assert!(playback.args.contains(&OsString::from(arg)));
            }
            assert!(matches!(playback.run().unwrap(), video::Outcome::Browser));
            script(&dir, "mpv", "exit 42");
            assert!(matches!(
                playback.run().unwrap(),
                video::Outcome::ExitApplication
            ));
            script(&dir, "mpv", "exit 1");
            assert!(playback.run().is_err());
            assert!(video::Playback::prepare_with(
                dir.join("missing").as_os_str(),
                &source,
                &token,
                1
            )
            .is_err());
            script(&dir, "mpv", "printf ' gpu only\\n'");
            assert!(video::Playback::prepare_with(mpv.as_os_str(), &source, &token, 1).is_err());
        }
    }
}
