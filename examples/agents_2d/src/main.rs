#![forbid(unsafe_code)]
// example binary, not simulation code
#![allow(clippy::print_stdout, clippy::print_stderr)]
//! The `agents_2d` host: a `winit` window (or `--headless`) around the [`agents_2d`] library.
//!
//! ```text
//! cargo run -p agents_2d --release -- [--agents N] [--seed N] [--headless [--ticks N]]
//! ```
//!
//! Headless mode prints `tick=… agents=… seekers=… checksum=0x…` and opens no window. Window
//! mode shows the world letterboxed; seekers chase the cursor, Space pauses, Escape quits.
//!
//! The simulation never sees a `winit` or `softbuffer` type. This host turns events into
//! [`Input`], owns time (`SystemClock` feeding a `FixedTimestep`), and decides when to step.

use core::num::NonZeroU32;
use core::time::Duration;
use std::process::ExitCode;
use std::rc::Rc;

use agents_2d::{Frame, Input, Sim, SimConfig, SimSummary, TICK_HZ, View, render, run};
use engine::prelude::*;
use softbuffer::{Context, Surface};
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalPosition;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key, NamedKey};
use winit::window::{Window, WindowId};

/// How often the title statistics refresh.
const TITLE_PERIOD: Duration = Duration::from_secs(1);

/// What the command line asked for.
#[derive(Debug, PartialEq, Eq)]
enum Mode {
    Window,
    Headless { ticks: u64 },
}

#[derive(Debug, PartialEq, Eq)]
struct Args {
    config: SimConfig,
    mode: Mode,
}

/// Parses `--agents N`, `--seed N`, `--headless` and `--ticks N` (only with `--headless`).
fn parse(args: &[String]) -> Result<Args, String> {
    let mut config = SimConfig::default();
    let mut headless = false;
    let mut ticks = None;
    let mut it = args.iter();
    while let Some(flag) = it.next() {
        let mut number = |name: &str| -> Result<u64, String> {
            let text = it
                .next()
                .ok_or_else(|| format!("`{name}` needs a number"))?;
            text.parse()
                .map_err(|_| format!("`{name}` needs a number, got `{text}`"))
        };
        match flag.as_str() {
            "--agents" => {
                let n = number("--agents")?;
                config.agents = usize::try_from(n)
                    .map_err(|_| format!("`--agents` is too large for this platform, got `{n}`"))?;
            }
            "--seed" => config.seed = number("--seed")?,
            "--ticks" => ticks = Some(number("--ticks")?),
            "--headless" => headless = true,
            other => return Err(format!("unknown argument `{other}`")),
        }
    }
    let mode = match (headless, ticks) {
        (true, ticks) => Mode::Headless {
            ticks: ticks.unwrap_or(600),
        },
        (false, None) => Mode::Window,
        (false, Some(_)) => return Err("`--ticks` requires `--headless`".to_owned()),
    };
    Ok(Args { config, mode })
}

fn summary_line(summary: &SimSummary) -> String {
    format!(
        "tick={} agents={} seekers={} checksum={:#018x}",
        summary.tick, summary.agents, summary.seekers, summary.checksum
    )
}

/// The simulation and everything that decides when and how it steps.
struct World {
    sim: Sim,
    view: View,
    timestep: FixedTimestep,
    clock: SystemClock,
    last_frame: Timestamp,
    last_title: Timestamp,
    cursor: Option<Vec2>,
    paused: bool,
    step_time: Duration,
    steps: u32,
}

impl World {
    fn new(config: &SimConfig, size: (u32, u32)) -> Self {
        let clock = SystemClock::new();
        let now = clock.now();
        let hz = NonZeroU32::new(TICK_HZ).expect("TICK_HZ is non-zero");
        Self {
            sim: Sim::new(config),
            view: View::fit(size.0, size.1),
            timestep: FixedTimestep::from_hz(hz),
            clock,
            last_frame: now,
            last_title: now,
            cursor: None,
            paused: false,
            step_time: Duration::ZERO,
            steps: 0,
        }
    }

    /// Runs the fixed steps owed since the previous frame (none while paused).
    fn advance(&mut self) {
        let now = self.clock.now();
        let elapsed = now.saturating_duration_since(self.last_frame);
        self.last_frame = now;
        if self.paused {
            return;
        }
        let input = Input {
            cursor: self.cursor.and_then(|c| self.view.cursor_to_world(c)),
        };
        for _tick in self.timestep.accumulate(elapsed) {
            let start = self.clock.now();
            self.sim.step(&input);
            self.step_time += self.clock.now().saturating_duration_since(start);
            self.steps += 1;
        }
    }

    /// The new window title, if a title period has passed since the last one.
    fn title_update(&mut self) -> Option<String> {
        let now = self.clock.now();
        if now.saturating_duration_since(self.last_title) < TITLE_PERIOD {
            return None;
        }
        self.last_title = now;
        let mean_ms = if self.steps == 0 {
            0.0
        } else {
            self.step_time.as_secs_f64() * 1000.0 / f64::from(self.steps)
        };
        self.step_time = Duration::ZERO;
        self.steps = 0;
        let summary = self.sim.summary();
        Some(format!(
            "agents_2d: {} agents, {} seekers, step {mean_ms:.3} ms",
            summary.agents, summary.seekers
        ))
    }
}

/// The window and its `softbuffer` surface, created once in `resumed`.
struct Gfx {
    window: Rc<Window>,
    surface: Surface<Rc<Window>, Rc<Window>>,
    surface_size: (u32, u32),
}

struct App {
    config: SimConfig,
    world: Option<World>,
    gfx: Option<Gfx>,
    /// The window's physical size, as last reported.
    size: (u32, u32),
    /// The first error from the windowing or presentation layer; it stops the loop.
    error: Option<String>,
}

impl App {
    fn new(config: SimConfig) -> Self {
        Self {
            config,
            world: None,
            gfx: None,
            size: (0, 0),
            error: None,
        }
    }

    fn fail(&mut self, event_loop: &ActiveEventLoop, message: String) {
        self.error = Some(message);
        event_loop.exit();
    }

    /// Runs one frame: steps the simulation, draws it and presents the result.
    fn frame(&mut self) -> Result<(), String> {
        let (Some(world), Some(gfx)) = (self.world.as_mut(), self.gfx.as_mut()) else {
            return Ok(());
        };
        let (width, height) = self.size;
        let (Some(w), Some(h)) = (NonZeroU32::new(width), NonZeroU32::new(height)) else {
            return Ok(()); // minimized: draw again once a non-zero size arrives
        };
        if gfx.surface_size != (width, height) {
            gfx.surface
                .resize(w, h)
                .map_err(|e| format!("resizing the surface: {e}"))?;
            gfx.surface_size = (width, height);
            world.view = View::fit(width, height);
        }

        world.advance();

        let mut buffer = gfx
            .surface
            .buffer_mut()
            .map_err(|e| format!("getting a buffer: {e}"))?;
        if let Some(mut frame) = Frame::new(&mut buffer, width, height) {
            render(&world.sim, &world.view, &mut frame);
        }
        buffer.present().map_err(|e| format!("presenting: {e}"))?;

        if let Some(title) = world.title_update() {
            gfx.window.set_title(&title);
        }
        gfx.window.request_redraw();
        Ok(())
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.gfx.is_some() {
            return; // already created; `resumed` can run again on some platforms
        }
        let window =
            match event_loop.create_window(Window::default_attributes().with_title("agents_2d")) {
                Ok(window) => Rc::new(window),
                Err(e) => return self.fail(event_loop, format!("creating the window: {e}")),
            };
        let surface = Context::new(Rc::clone(&window))
            .and_then(|context| Surface::new(&context, Rc::clone(&window)));
        let surface = match surface {
            Ok(surface) => surface,
            Err(e) => return self.fail(event_loop, format!("creating the surface: {e}")),
        };
        let size = window.inner_size();
        self.size = (size.width, size.height);
        self.world = Some(World::new(&self.config, self.size));
        window.request_redraw();
        self.gfx = Some(Gfx {
            window,
            surface,
            surface_size: (0, 0),
        });
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                self.size = (size.width, size.height);
                if let Some(gfx) = &self.gfx {
                    gfx.window.request_redraw();
                }
            }
            WindowEvent::CursorMoved {
                position: PhysicalPosition { x, y },
                ..
            } => {
                if let Some(world) = &mut self.world {
                    #[allow(clippy::cast_possible_truncation, reason = "pixel positions fit f32")]
                    let position = Vec2::new(x as f32, y as f32);
                    world.cursor = Some(position);
                }
            }
            WindowEvent::CursorLeft { .. } => {
                if let Some(world) = &mut self.world {
                    world.cursor = None;
                }
            }
            WindowEvent::KeyboardInput { event, .. }
                if event.state == ElementState::Pressed && !event.repeat =>
            {
                match event.logical_key {
                    Key::Named(NamedKey::Escape) => event_loop.exit(),
                    Key::Named(NamedKey::Space) => {
                        if let Some(world) = &mut self.world {
                            world.paused = !world.paused;
                        }
                    }
                    _ => {}
                }
            }
            WindowEvent::RedrawRequested => {
                if let Err(message) = self.frame() {
                    self.fail(event_loop, message);
                }
            }
            _ => {}
        }
    }
}

fn run_window(config: SimConfig) -> Result<(), String> {
    let event_loop = EventLoop::new().map_err(|e| format!("creating the event loop: {e}"))?;
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = App::new(config);
    event_loop
        .run_app(&mut app)
        .map_err(|e| format!("event loop: {e}"))?;
    app.error.map_or(Ok(()), Err)
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args = match parse(&args) {
        Ok(args) => args,
        Err(message) => {
            eprintln!("agents_2d: {message}");
            return ExitCode::FAILURE;
        }
    };
    match args.mode {
        Mode::Headless { ticks } => {
            println!("{}", summary_line(&run(&args.config, ticks)));
            ExitCode::SUCCESS
        }
        Mode::Window => match run_window(args.config) {
            Ok(()) => ExitCode::SUCCESS,
            Err(message) => {
                eprintln!("agents_2d: {message}");
                ExitCode::FAILURE
            }
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_strs(args: &[&str]) -> Result<Args, String> {
        let owned: Vec<String> = args.iter().map(|s| (*s).to_owned()).collect();
        parse(&owned)
    }

    #[test]
    fn parse_defaults() {
        assert_eq!(
            parse_strs(&[]),
            Ok(Args {
                config: SimConfig::default(),
                mode: Mode::Window
            })
        );
    }

    #[test]
    fn parse_agents_and_seed() {
        let args = parse_strs(&["--agents", "5", "--seed", "7"]).unwrap();
        assert_eq!(args.config, SimConfig { seed: 7, agents: 5 });
        assert_eq!(args.mode, Mode::Window);
    }

    #[test]
    fn parse_headless() {
        let ticks = |args: &[&str]| parse_strs(args).unwrap().mode;
        assert_eq!(ticks(&["--headless"]), Mode::Headless { ticks: 600 });
        assert_eq!(
            ticks(&["--headless", "--ticks", "10"]),
            Mode::Headless { ticks: 10 }
        );
        assert_eq!(
            ticks(&["--ticks", "10", "--headless"]),
            Mode::Headless { ticks: 10 }
        );
    }

    #[test]
    fn parse_errors_name_the_problem() {
        let err = |args: &[&str], needles: &[&str]| {
            let message = parse_strs(args).unwrap_err();
            for needle in needles {
                assert!(
                    message.contains(needle),
                    "{message:?} should contain {needle:?}"
                );
            }
        };
        err(&["--bogus"], &["--bogus"]);
        err(&["--agents"], &["--agents"]);
        err(&["--agents", "x"], &["--agents", "x"]);
        err(&["--seed", "-1"], &["--seed", "-1"]);
        err(&["--ticks", "10"], &["--ticks", "--headless"]);
    }

    #[test]
    fn summary_line_format() {
        let summary = SimSummary {
            tick: 600,
            agents: 3,
            seekers: 1,
            checksum: 0xAB,
        };
        assert_eq!(
            summary_line(&summary),
            "tick=600 agents=3 seekers=1 checksum=0x00000000000000ab"
        );
    }
}
