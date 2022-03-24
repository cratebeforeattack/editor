use std::collections::VecDeque;
use std::mem::swap;

#[derive(Clone)]
pub enum ProfileMarker {
    OpenBlock(&'static str),
    CloseBlock,
}

pub struct Profiler {
    pub enabled: bool,
    markers: Vec<(ProfileMarker, f64)>,
    pub last_frame_markers: Vec<(ProfileMarker, f64)>,
}

impl Profiler {
    pub fn new() -> Self {
        Profiler {
            enabled: true,
            markers: Vec::new(),
            last_frame_markers: Vec::new(),
        }
    }
    pub fn begin_frame(&mut self) {
        if !self.enabled {
            return;
        }
        swap(&mut self.markers, &mut self.last_frame_markers);
        self.markers.clear();
    }
    pub fn open_block(&mut self, name: &'static str) {
        if !self.enabled {
            return;
        }
        self.markers
            .push((ProfileMarker::OpenBlock(name), miniquad::date::now()));
    }
    pub fn close_block(&mut self) {
        if !self.enabled {
            return;
        }
        self.markers
            .push((ProfileMarker::CloseBlock, miniquad::date::now()));
    }

    pub fn ui_profiler(
        &self,
        ui: &mut rimui::UI,
        rows: rimui::AreaRef,
        title: &str,
        font: Option<rimui::FontKey>,
    ) {
        use rimui::*;
        let bar = progress()
            .align(Right)
            .color(Some([80, 80, 80, 255]))
            .min_size([100, 16]);

        let markers = &self.last_frame_markers;
        let total_time = markers.last().map(|l| l.1).unwrap_or(0.016666)
            - markers.first().map(|l| l.1).unwrap_or(0.0);
        let h = ui.add(rows, hbox());
        ui.add(h, label(title).align(Left).expand(true).font(font));
        ui.add(
            h,
            label(&format!("{:.2} ms", total_time * 1000.0)).font(font),
        );
        // CPU
        let mut stack = Vec::new();
        let indent = 16;
        for i in 0..markers.len() {
            let (marker, t) = &markers[i];
            match marker {
                ProfileMarker::OpenBlock(name) => {
                    let h = ui.add(rows, hbox());
                    ui.add(
                        h,
                        spacer()
                            .min_size([indent * (stack.len() + 1) as u16, 0])
                            .expand(false),
                    );
                    ui.add(h, label(name).align(Left).expand(true).font(font));
                    stack.push((h, *t));
                }
                ProfileMarker::CloseBlock => {
                    let (h, start_time) = stack.pop().unwrap();
                    let delta = *t - start_time;
                    let s = ui.add(h, rimui::stack());
                    if matches!(
                        markers.get(i - 1),
                        Some((ProfileMarker::OpenBlock { .. }, _))
                    ) {
                        ui.add(s, bar.progress(delta as f32 / total_time as f32));
                    }
                    ui.add(
                        s,
                        label(&format!("{:.2} ms", delta * 1000.0))
                            .font(font)
                            .align(Right),
                    );
                }
            }
        }
    }

    pub(crate) fn total_duration(&self) -> Option<f64> {
        if self.last_frame_markers.len() >= 2 {
            Some(
                self.last_frame_markers.last().unwrap().1
                    - self.last_frame_markers.first().unwrap().1,
            )
        } else {
            None
        }
    }
}

pub struct GPUProfilerFrame {
    elapsed_queries: Vec<miniquad::ElapsedQuery>,
    markers: Vec<ProfileMarker>,
}

pub struct GPUProfiler {
    frame_queue: VecDeque<GPUProfilerFrame>,
    frame_pool: Vec<GPUProfilerFrame>,
    available_frame: Option<GPUProfilerFrame>,
    frame: GPUProfilerFrame,
    timings: Vec<u64>,
    depth: usize,
    pub enabled: bool,
    pub supported: bool,
    pub total_time: u64,
}

#[allow(dead_code)]
impl GPUProfiler {
    pub fn new() -> GPUProfiler {
        GPUProfiler {
            frame_queue: VecDeque::new(),
            frame_pool: Vec::new(),
            timings: Vec::new(),
            frame: GPUProfilerFrame {
                markers: Vec::new(),
                elapsed_queries: Vec::new(),
            },
            available_frame: None,
            total_time: 0,
            depth: 0,
            enabled: false,
            supported: miniquad::ElapsedQuery::is_supported(),
        }
    }
    pub fn push_marker(&mut self, m: ProfileMarker) {
        let frame = &mut self.frame;
        let markers = &mut frame.markers;

        let index = markers.len();
        let queries = &mut frame.elapsed_queries;
        if let Some(last_query) = queries.get_mut(index.wrapping_sub(1)) {
            last_query.end_query();
        }
        let q = if let Some(q) = queries.get_mut(index) {
            q
        } else {
            queries.push(miniquad::ElapsedQuery::new());
            queries.last_mut().unwrap()
        };
        if matches!(&m, ProfileMarker::OpenBlock { .. }) || self.depth > 0 {
            q.begin_query();
        }
        markers.push(m);
    }

    pub fn open_block(&mut self, name: &'static str) {
        if !self.enabled {
            return;
        }
        self.push_marker(ProfileMarker::OpenBlock(name));
        self.depth += 1;
    }

    pub fn close_block(&mut self) {
        if !self.enabled {
            return;
        }
        self.depth -= 1;
        self.push_marker(ProfileMarker::CloseBlock);
    }

    pub fn queue_len(&self) -> usize {
        self.frame_queue.len()
    }

    pub fn marker_timings(&self) -> (&[ProfileMarker], &[u64]) {
        if let Some(frame) = &self.available_frame {
            (frame.markers.as_slice(), self.timings.as_slice())
        } else {
            (&[], &[])
        }
    }

    pub fn begin_frame(&mut self) {
        if !self.enabled {
            return;
        }
        let mut frame = self.frame_pool.pop().unwrap_or_else(|| GPUProfilerFrame {
            markers: Vec::new(),
            elapsed_queries: Vec::new(),
        });
        frame.markers.clear();
        self.frame_queue
            .push_back(std::mem::replace(&mut self.frame, frame));

        let mut available_frame = None;
        while let Some(oldest) = self.frame_queue.front() {
            let mut available = true;
            for i in 0..oldest.elapsed_queries.len().max(1) - 1 {
                if !oldest.elapsed_queries[i].is_available() {
                    available = false;
                    break;
                }
            }
            if !available {
                break;
            }

            if let Some(available_frame) = available_frame.take() {
                self.frame_pool.push(available_frame);
            }
            available_frame = self.frame_queue.pop_front();
        }

        if let Some(frame) = available_frame {
            self.total_time = 0;
            if let Some(old_frame) = self.available_frame.take() {
                self.frame_pool.push(old_frame);
            }
            // read queries of the available frame
            self.timings.clear();
            let num = frame.markers.len();
            self.timings.reserve(num);

            let queries = &frame.elapsed_queries;
            for i in 0..num.max(1) - 1 {
                self.timings.push(self.total_time);
                let elapsed = queries[i].get_result();
                self.total_time += elapsed;
            }
            self.timings.push(self.total_time);

            self.available_frame = Some(frame);
        }
    }
}
