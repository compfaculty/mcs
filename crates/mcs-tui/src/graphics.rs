//! Kitty-only encoding worker. Protocol preparation is never done in draw().
use mcs_media::{DynamicImage, ImageView, Kind, Pdf};
use ratatui::layout::Size;
use ratatui_image::{
    protocol::{kitty::Kitty, Protocol},
    FontSize,
};
use std::{
    io,
    path::PathBuf,
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc::{self, Receiver, SyncSender, TrySendError},
        Arc,
    },
    thread,
};

pub struct Frame {
    pub columns: u16,
    pub rows: u16,
    pub protocol: Protocol,
    placement: Size,
    pub id: u32,
    pub page: u32,
    pub pages: u32,
    pub message: String,
}
// ratatui-image 11.1 encodes the bitmap and placeholders, but its public Kitty
// constructor omits c/r on the virtual placement. Supply them on the initial
// transmission so a budget-downsampled raster still fills the intended cells.
impl ratatui::widgets::Widget for &Frame {
    fn render(self, area: ratatui::layout::Rect, buffer: &mut ratatui::buffer::Buffer) {
        ratatui_image::Image::new(&self.protocol).render(area, buffer);
        if let Some(cell) = buffer.cell_mut((area.x, area.y)) {
            if cell.symbol().contains("a=T,U=1,f=32,") {
                let symbol = cell.symbol().replacen(
                    "a=T,U=1,f=32,",
                    &format!(
                        "a=T,U=1,c={},r={},f=32,",
                        self.placement.width, self.placement.height
                    ),
                    1,
                );
                cell.set_symbol(&symbol);
            }
        }
    }
}

#[derive(Clone)]
pub struct Job {
    pub compression: bool,
    pub path: PathBuf,
    pub kind: Kind,
    pub page: u32,
    pub page_delta: i32,
    pub view: ImageView,
    pub columns: u16,
    pub rows: u16,
    pub font: FontSize,
    pub search: Option<(String, bool, bool)>,
    pub reload: bool,
}
pub enum Event {
    Frame(Frame),
    Progress(u32),
    Error(String),
}
struct Envelope {
    generation: u64,
    job: Job,
}
pub struct Client {
    requests: SyncSender<Envelope>,
    results: Receiver<(u64, Event)>,
    token: Arc<AtomicU64>,
    queued: Option<Envelope>,
    pub busy: bool,
}
struct Document {
    path: PathBuf,
    pdf: Option<Pdf>,
    source: Option<DynamicImage>,
    page: u32,
}
impl Document {
    fn frame(
        &mut self,
        job: Job,
        token: &AtomicU64,
        generation: u64,
        progress: impl FnMut(u32),
    ) -> io::Result<Frame> {
        if self.path != job.path || job.reload {
            self.pdf = None;
            self.source = None;
            self.page = 0;
            self.path = job.path.clone();
            if job.kind == Kind::Pdf {
                self.pdf = Some(Pdf::open(&job.path, token, generation)?);
            }
        }
        let pages = self.pdf.as_ref().map(|pdf| pdf.pages).unwrap_or(1);
        let mut page =
            (i64::from(job.page) + i64::from(job.page_delta)).clamp(1, i64::from(pages)) as u32;
        let mut message = String::new();
        if let Some((query, reverse, repeat)) = &job.search {
            if let Some(pdf) = &self.pdf {
                match pdf.search(
                    mcs_media::PdfSearch {
                        query,
                        current: page,
                        reverse: *reverse,
                        repeat: *repeat,
                    },
                    token,
                    generation,
                    progress,
                )? {
                    Some((found, snippet)) => {
                        page = found;
                        message = format!("Match: {}", mcs_viewer::display_text(&snippet));
                    }
                    None => message = "No match in searchable PDF text (no OCR)".into(),
                }
            }
        }
        if token.load(Ordering::Relaxed) != generation {
            return Err(io::Error::other("Cancelled"));
        }
        if self.source.is_none() || self.page != page {
            self.source = None; // Release old page before decoding its replacement.
            self.source = Some(if let Some(pdf) = &self.pdf {
                pdf.render(page, token, generation)?
            } else {
                mcs_media::load_image(&job.path)?
            });
            self.page = page;
        }
        let source = self.source.as_ref().expect("loaded image");
        let rendered = mcs_media::render_viewport(source, job.view)?;
        if rendered.image.width() < rendered.display_width
            || rendered.image.height() < rendered.display_height
        {
            if !message.is_empty() {
                message.push_str(" | ");
            }
            message.push_str("Display scaled within raster budget");
        }
        if token.load(Ordering::Relaxed) != generation {
            return Err(io::Error::other("Cancelled"));
        }
        let columns = rendered
            .display_width
            .div_ceil(u32::from(job.font.width))
            .min(u32::from(job.columns)) as u16;
        let rows = rendered
            .display_height
            .div_ceil(u32::from(job.font.height))
            .min(u32::from(job.rows)) as u16;
        let id = 0x4d00_0000
            | ((std::process::id()
                .wrapping_mul(257)
                .wrapping_add(generation as u32))
                & 0x00ff_ffff);
        let protocol = Protocol::Kitty(
            Kitty::new(
                rendered.image,
                Size::new(columns, rows),
                id,
                false,
                job.compression,
            )
            .map_err(|error| io::Error::other(error.to_string()))?,
        );
        Ok(Frame {
            columns: job.columns,
            rows: job.rows,
            protocol,
            placement: Size::new(columns, rows),
            id,
            page,
            pages,
            message,
        })
    }
}
impl Client {
    pub fn new() -> io::Result<Self> {
        let (requests, receiver) = mpsc::sync_channel::<Envelope>(1);
        let (sender, results) = mpsc::sync_channel(1);
        let token = Arc::new(AtomicU64::new(0));
        let cancel = token.clone();
        thread::Builder::new()
            .name("mcs-graphics".into())
            .spawn(move || {
                let mut document = Document {
                    path: PathBuf::new(),
                    pdf: None,
                    source: None,
                    page: 0,
                };
                while let Ok(envelope) = receiver.recv() {
                    let generation = envelope.generation;
                    if cancel.load(Ordering::Relaxed) != generation {
                        continue;
                    }
                    let result = document.frame(envelope.job, &cancel, generation, |page| {
                        let _ = sender.try_send((generation, Event::Progress(page)));
                    });
                    if cancel.load(Ordering::Relaxed) == generation {
                        let event = result
                            .map(Event::Frame)
                            .unwrap_or_else(|error| Event::Error(error.to_string()));
                        if sender.send((generation, event)).is_err() {
                            break;
                        }
                    }
                }
            })?;
        Ok(Self {
            requests,
            results,
            token,
            queued: None,
            busy: false,
        })
    }
    pub fn submit(&mut self, job: Job) {
        let generation = self.token.fetch_add(1, Ordering::Relaxed) + 1;
        self.queued = Some(Envelope { generation, job });
        self.busy = true;
        self.flush();
    }
    fn flush(&mut self) {
        if let Some(job) = self.queued.take() {
            match self.requests.try_send(job) {
                Ok(()) => {}
                Err(TrySendError::Full(job)) => self.queued = Some(job),
                Err(TrySendError::Disconnected(_)) => self.busy = false,
            }
        }
    }
    pub fn poll(&mut self) -> Option<Event> {
        let mut result = None;
        while let Ok((generation, event)) = self.results.try_recv() {
            if generation == self.token.load(Ordering::Relaxed) {
                if !matches!(event, Event::Progress(_)) {
                    self.busy = false;
                }
                result = Some(event);
            }
        }
        self.flush();
        result
    }
    pub fn cancel(&mut self) {
        self.token.fetch_add(1, Ordering::Relaxed);
        self.queued = None;
        self.busy = false;
    }
}
impl Drop for Client {
    fn drop(&mut self) {
        self.cancel();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn retina_raster_keeps_full_sized_kitty_placement() {
        let path = PathBuf::from("owned-fixture.png");
        let mut document = Document {
            path: path.clone(),
            pdf: None,
            source: Some(DynamicImage::new_rgb8(8, 8)),
            page: 1,
        };
        let job = Job {
            compression: false,
            path,
            kind: Kind::Image,
            page: 1,
            page_delta: 0,
            view: ImageView {
                width: 4000,
                height: 3200,
                zoom: 0,
                pan_x: 0,
                pan_y: 0,
            },
            columns: 200,
            rows: 80,
            font: FontSize::new(20, 40),
            search: None,
            reload: false,
        };
        let frame = document.frame(job, &AtomicU64::new(1), 1, |_| {}).unwrap();
        assert_eq!(frame.protocol.size(), Size::new(160, 80));
        let area = ratatui::layout::Rect::new(0, 0, 200, 80);
        let mut buffer = ratatui::buffer::Buffer::empty(area);
        ratatui::widgets::Widget::render(&frame, area, &mut buffer);
        let first = buffer.cell((0, 0)).unwrap().symbol();
        assert!(first.contains("a=T,U=1,c=160,r=80,f=32,"));
        // Small source pixels are scaled by Kitty, never expanded in software.
        assert!(first.contains("s=8,v=8,"));
        assert!(frame.message.contains("raster budget"));
        ratatui::widgets::Widget::render(&frame, area, &mut buffer);
        assert!(!buffer.cell((0, 0)).unwrap().symbol().contains("a=T"));
    }
    #[test]
    fn graphics_cancellation_rejects_late_results() {
        let (requests, _receiver) = mpsc::sync_channel(1);
        let (sender, results) = mpsc::sync_channel(1);
        let mut client = Client {
            requests,
            results,
            token: Arc::new(AtomicU64::new(2)),
            queued: None,
            busy: true,
        };
        sender
            .send((1, Event::Error("stale decoder result".into())))
            .ok()
            .unwrap();
        assert!(client.poll().is_none());
        client.cancel();
        assert!(!client.busy);
    }
}

#[cfg(test)]
mod benchmarks {
    use super::*;
    use std::{
        fs,
        io::Write,
        time::{Instant, SystemTime, UNIX_EPOCH},
    };

    // A fixed public-content fixture, not a PDF parser or application document writer.
    fn pdf_fixture() -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "mcs-perf-{}-{}.pdf",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let mut text = String::from("BT /F1 12 Tf 40 800 Td 14 TL\n");
        for _ in 0..48 {
            text.push_str("(Generated viewer performance fixture 0123456789) Tj T*\n");
        }
        text.push_str("ET\n");
        let objects = ["<< /Type /Catalog /Pages 2 0 R >>".to_string(),
            "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".into(),
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] /Resources << /Font << /F1 4 0 R >> >> /Contents 5 0 R >>".into(),
            "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".into(),
            format!("<< /Length {} >>\nstream\n{text}endstream", text.len())];
        let mut pdf = b"%PDF-1.4\n".to_vec();
        let mut offsets = vec![0];
        for (index, object) in objects.iter().enumerate() {
            offsets.push(pdf.len());
            write!(pdf, "{} 0 obj\n{}\nendobj\n", index + 1, object).unwrap();
        }
        let xref = pdf.len();
        write!(pdf, "xref\n0 6\n0000000000 65535 f \n").unwrap();
        for offset in offsets.iter().skip(1) {
            writeln!(pdf, "{offset:010} 00000 n ").unwrap();
        }
        write!(
            pdf,
            "trailer\n<< /Size 6 /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n"
        )
        .unwrap();
        fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .unwrap()
            .write_all(&pdf)
            .unwrap();
        path
    }
    #[test]
    #[ignore = "explicit local pixel-equivalence check; requires installed Poppler"]
    fn pdf_ppm_matches_png_pixels() {
        let path = pdf_fixture();
        let token = AtomicU64::new(1);
        let pdf = Pdf::open(&path, &token, 1).unwrap();
        let ppm = pdf.render(1, &token, 1).unwrap();
        let args = vec![
            "-f".into(),
            "1".into(),
            "-l".into(),
            "1".into(),
            "-singlefile".into(),
            "-scale-to".into(),
            "2048".into(),
            "-png".into(),
            path.into_os_string(),
        ];
        let bytes = mcs_media::process::capture(
            std::ffi::OsStr::new("pdftoppm"),
            &args,
            mcs_media::MAX_ENCODED as usize,
            std::time::Duration::from_secs(10),
            &token,
            1,
        )
        .unwrap();
        let png = mcs_media::decode(std::io::Cursor::new(bytes)).unwrap();
        assert_eq!((ppm.width(), ppm.height()), (png.width(), png.height()));
        assert!(
            ppm.to_rgb8().as_raw() == png.to_rgb8().as_raw(),
            "Poppler RGB transfer changed pixels"
        );
    }
    #[test]
    #[ignore = "explicit local performance experiment; requires installed Poppler"]
    fn benchmark_pdf_open() {
        let path = pdf_fixture();
        let token = AtomicU64::new(1);
        for sample in 0..3 {
            let start = Instant::now();
            let pdf = Pdf::open(&path, &token, 1).unwrap();
            let metadata = start.elapsed();
            let start = Instant::now();
            let source = pdf.render(1, &token, 1).unwrap();
            let decode = start.elapsed();
            let start = Instant::now();
            let rendered = mcs_media::render_viewport(
                &source,
                ImageView {
                    width: 4000,
                    height: 3000,
                    zoom: 0,
                    pan_x: 0,
                    pan_y: 0,
                },
            )
            .unwrap();
            let resize = start.elapsed();
            let size = Size::new(
                rendered.display_width.div_ceil(20) as u16,
                rendered.display_height.div_ceil(40) as u16,
            );
            for compress in [false, true] {
                let pixels = rendered.image.clone();
                let start = Instant::now();
                let protocol =
                    Protocol::Kitty(Kitty::new(pixels, size, 100, false, compress).unwrap());
                let encode = start.elapsed();
                let frame = Frame {
                    columns: 200,
                    rows: 75,
                    protocol,
                    placement: size,
                    id: 100,
                    page: 1,
                    pages: 1,
                    message: String::new(),
                };
                let area = ratatui::layout::Rect::new(0, 0, 200, 75);
                let mut buffer = ratatui::buffer::Buffer::empty(area);
                let start = Instant::now();
                ratatui::widgets::Widget::render(&frame, area, &mut buffer);
                let draw = start.elapsed();
                let bytes: usize = buffer.content.iter().map(|cell| cell.symbol().len()).sum();
                println!("profile={} sample={} compressed={} metadata_ms={:.2} render_decode_ms={:.2} resize_ms={:.2} encode_ms={:.2} buffer_ms={:.2} encoded_bytes={} raster={}x{}", if cfg!(debug_assertions) { "debug" } else { "release" }, sample, compress, metadata.as_secs_f64()*1000., decode.as_secs_f64()*1000., resize.as_secs_f64()*1000., encode.as_secs_f64()*1000., draw.as_secs_f64()*1000., bytes, rendered.image.width(), rendered.image.height());
            }
        }
    }
}
