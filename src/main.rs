mod grid;
pub mod gridgen;
pub mod sim;
mod style;

use futures::{
    channel::mpsc::{Receiver, Sender},
    prelude::*,
};
use iced::{
    button, executor, slider, time, Align, Application, Button, Column, Command, Container,
    Element, HorizontalAlignment, Length, Radio, Row, Settings, Slider, Subscription, Text,
    VerticalAlignment,
};
use rand::SeedableRng;
use std::time::Duration;

std::thread_local! {
    pub static RNG: rand_chacha::ChaCha8Rng = rand_chacha::ChaCha8Rng::from_entropy();
}

unsafe fn rng() -> &'static mut rand_chacha::ChaCha8Rng {
    RNG.with(|rng| std::mem::transmute(rng as *const rand_chacha::ChaCha8Rng))
}

pub fn main() {
    EvonomicsWorld::run(Settings {
        antialiasing: true,
        ..Settings::default()
    })
}

struct EvonomicsWorld {
    grid: Option<grid::Grid>,
    sim_tx: Option<Sender<sim::ToSim>>,
    run_simulation_button: button::State,
    load_save_button: button::State,
    save_simulation_button: button::State,
    toggle_spawn_rate_type_button: button::State,
    is_inverse_rate_type: bool,
    spawn_slider: slider::State,
    spawn_rate: f32,
    spawn_chance: f64,
    toggle_run_button: button::State,
    toggle_grid_button: button::State,
    frame_rate_slider: slider::State,
    frames_per_second: usize,
    ms_per_frame: usize,
    speed_slider: slider::State,
    speed: usize,
    cell_count: usize,
    dimension_slider: slider::State,
    width: usize,
    grid_openness_slider: slider::State,
    openness: usize,
    menu_state: MenuState,
    is_running_sim: bool,
    next_speed: Option<usize>,
    aspect_ratio: AspectRatio,
    total_tick_count: u64,
}

enum MenuState {
    MainMenu,
    SimMenu,
}

impl std::default::Default for MenuState {
    fn default() -> MenuState {
        MenuState::MainMenu
    }
}

#[derive(Debug)]
enum Message {
    FromSim(sim::FromSim, Receiver<sim::FromSim>),
    SimView,
    MainView,
    SpeedChanged(f32),
    FrameRateChanged(f32),
    SpawnRateChanged(f32),
    ToggleRateType,
    DimensionSet(f32),
    AspectChanged(AspectRatio),
    OpennessSet(f32),
    ToggleSim,
    ToggleGrid,
    Tick,
    Null,
}

impl Clone for Message {
    fn clone(&self) -> Self {
        match self {
            Self::SimView => Self::SimView,
            Self::MainView => Self::MainView,
            Self::ToggleSim => Self::ToggleSim,
            Self::ToggleGrid => Self::ToggleGrid,
            Self::Tick => Self::Tick,
            Self::ToggleRateType => Self::ToggleRateType,
            Self::SpawnRateChanged(spwn) => Message::SpawnRateChanged(spwn.clone()),
            Self::AspectChanged(aspect) => Message::AspectChanged(aspect.clone()),
            Self::SpeedChanged(spd) => Message::SpeedChanged(spd.clone()),
            Self::FrameRateChanged(rt) => Message::FrameRateChanged(rt.clone()),
            Self::DimensionSet(dm) => Message::DimensionSet(dm.clone()),
            Self::OpennessSet(openness) => Message::OpennessSet(openness.clone()),
            _ => panic!("do not try to clone messages with data in them"),
        }
    }
}

fn reciever_command(rx: Receiver<sim::FromSim>) -> Command<Message> {
    Command::perform(rx.into_future(), |(item, stream)| {
        item.map(|item| Message::FromSim(item, stream))
            .unwrap_or(Message::Null)
    })
}

const SPAWN_CURVE: f64 = 0.000000001;

fn spawn_rate(
    is_inverse_rate_type: bool,
    cell_count: usize,
    height: usize,
    spawn_rate: f32,
) -> f64 {
    if is_inverse_rate_type {
        spawn_rate as f64 / ((cell_count + 1) * height) as f64
    } else {
        (SPAWN_CURVE.powf(1.0 - spawn_rate as f64) - SPAWN_CURVE) / (1.0 - SPAWN_CURVE)
    }
}

impl<'a> Application for EvonomicsWorld {
    // application produced messages
    type Message = Message;
    // run commands and subscriptions
    type Executor = executor::Default;
    // initialization data for application
    type Flags = ();

    fn new(_: ()) -> (EvonomicsWorld, Command<Self::Message>) {
        const INITIAL_SPAWN_RATE: f32 = 0.5;
        const INITIAL_IS_INVERSE_RATE: bool = true;
        const INITIAL_WIDTH: usize = 512;
        const INITIAL_ASPECT: AspectRatio = AspectRatio::SixteenToTen;
        (
            EvonomicsWorld {
                grid: None,
                sim_tx: None,
                run_simulation_button: Default::default(),
                load_save_button: Default::default(),
                save_simulation_button: Default::default(),
                toggle_spawn_rate_type_button: Default::default(),
                is_inverse_rate_type: INITIAL_IS_INVERSE_RATE,
                spawn_slider: Default::default(),
                spawn_rate: INITIAL_SPAWN_RATE,
                spawn_chance: spawn_rate(
                    INITIAL_IS_INVERSE_RATE,
                    0,
                    INITIAL_ASPECT.get_height(INITIAL_WIDTH),
                    INITIAL_SPAWN_RATE,
                ),
                toggle_run_button: Default::default(),
                toggle_grid_button: Default::default(),
                speed_slider: Default::default(),
                speed: 1,
                cell_count: 0,
                frame_rate_slider: Default::default(),
                frames_per_second: 1000 / 66,
                ms_per_frame: 66,
                dimension_slider: Default::default(),
                width: INITIAL_WIDTH,
                grid_openness_slider: Default::default(),
                openness: 1,
                menu_state: MenuState::MainMenu,
                is_running_sim: false,
                next_speed: None,
                aspect_ratio: INITIAL_ASPECT,
                total_tick_count: 0,
            },
            Command::none(),
        )
    }

    fn title(&self) -> String {
        String::from("Evonomics")
    }

    // handles user interactions
    fn update(&mut self, message: Self::Message) -> Command<Self::Message> {
        match message {
            Message::FromSim(from_sim, stream) => {
                match from_sim {
                    sim::FromSim::View(view) => match self.grid {
                        Some(ref mut grd) => {
                            self.cell_count = view.cells;
                            grd.update(view.into())
                        }
                        None => {}
                    },
                }
                return reciever_command(stream);
            }
            Message::AspectChanged(new_aspect) => {
                self.aspect_ratio = new_aspect;
            }
            Message::OpennessSet(new_openness) => {
                self.openness = new_openness as usize;
            }
            Message::SpawnRateChanged(new_rate) => {
                self.spawn_rate = new_rate;
                self.spawn_chance = spawn_rate(
                    self.is_inverse_rate_type,
                    self.cell_count,
                    self.aspect_ratio.get_height(self.width),
                    self.spawn_rate,
                );
                match self.sim_tx {
                    Some(ref mut tx) => {
                        // If the channel is full, dont send it.
                        tx.try_send(sim::ToSim::SetSpawnChance(self.spawn_chance))
                            .ok();
                    }
                    None => {}
                }
            }
            Message::ToggleRateType => {
                self.is_inverse_rate_type = !self.is_inverse_rate_type;
                self.spawn_chance = spawn_rate(
                    self.is_inverse_rate_type,
                    self.cell_count,
                    self.aspect_ratio.get_height(self.width),
                    self.spawn_rate,
                );
                match self.sim_tx {
                    Some(ref mut tx) => {
                        // If the channel is full, dont send it.
                        tx.try_send(sim::ToSim::SetSpawnChance(self.spawn_chance))
                            .ok();
                    }
                    None => {}
                }
            }
            Message::SimView => {
                self.menu_state = MenuState::SimMenu;
                // self.is_running_sim = true;

                let (sim_tx, sim_rx, sim_runner) = sim::run_sim(
                    3,
                    3,
                    self.width,
                    self.aspect_ratio.get_height(self.width),
                    self.openness,
                );

                self.sim_tx = Some(sim_tx);
                self.grid = Some(grid::Grid::new(
                    self.width,
                    self.aspect_ratio.get_height(self.width),
                ));

                return Command::batch(vec![
                    Command::perform(sim_runner, |_| Message::Null),
                    reciever_command(sim_rx),
                ]);
            }
            Message::MainView => {
                self.menu_state = MenuState::MainMenu;
                self.is_running_sim = false;
            }
            Message::FrameRateChanged(new_rate) => {
                self.frames_per_second = new_rate as usize;
                self.ms_per_frame = (1000.0 / new_rate) as usize;
            }
            Message::SpeedChanged(new_speed) => {
                self.speed = new_speed as usize;
            }
            Message::DimensionSet(new_dim) => {
                self.width = new_dim as usize;
            }
            Message::ToggleSim => {
                self.is_running_sim = !self.is_running_sim;
            }
            Message::ToggleGrid => match self.grid {
                Some(ref mut grd) => grd.toggle_lines(),
                None => {}
            },
            Message::Tick => {
                match self.sim_tx {
                    Some(ref mut tx) => {
                        // If the channel is full, dont send it.
                        match tx.try_send(sim::ToSim::Tick(self.speed)).ok() {
                            Some(_) => {
                                self.total_tick_count += self.speed as u64;
                            }
                            None => {}
                        }
                        self.update(Message::SpawnRateChanged(self.spawn_rate));
                    }
                    None => {}
                }
            }
            Message::Null => {}
        }
        Command::none()
    }

    // queue tick in update function regularly
    fn subscription(&self) -> Subscription<Message> {
        if self.is_running_sim {
            time::every(Duration::from_millis(self.ms_per_frame as u64)).map(|_| Message::Tick)
        } else {
            Subscription::none()
        }
    }

    fn view(&mut self) -> Element<Self::Message> {
        let speed = self.next_speed.unwrap_or(self.speed);

        match self.menu_state {
            MenuState::MainMenu => {
                let new_run_column = Column::new()
                    .spacing(10)
                    .max_width(style::MAIN_MENU_COLLUMN_WIDTH)
                    .align_items(Align::Center)
                    .push(
                        Button::new(
                            &mut self.run_simulation_button,
                            Text::new("Run Simulation")
                                .horizontal_alignment(HorizontalAlignment::Center),
                        )
                        .style(style::Theme::Default)
                        .min_width(style::MAIN_MENU_COLLUMN_WIDTH)
                        .on_press(Message::SimView),
                    )
                    .push(
                        Row::new()
                            .push(Radio::new(
                                AspectRatio::OneToOne,
                                "1:1",
                                Some(self.aspect_ratio),
                                Message::AspectChanged,
                            ))
                            .push(Radio::new(
                                AspectRatio::SixteenToTen,
                                "16:10",
                                Some(self.aspect_ratio),
                                Message::AspectChanged,
                            )),
                    )
                    .push(
                        Slider::new(
                            &mut self.grid_openness_slider,
                            0.0..=10.0,
                            self.openness as f32,
                            Message::OpennessSet,
                        )
                        .style(style::Theme::Default),
                    )
                    .push(
                        Text::new(format!(
                            "Maze Openness {:<4} (Increase to errode walls.)",
                            self.openness,
                        ))
                        .size(16)
                        .vertical_alignment(VerticalAlignment::Bottom)
                        .horizontal_alignment(HorizontalAlignment::Center)
                        .width(Length::Fill),
                    )
                    .push(
                        Slider::new(
                            &mut self.dimension_slider,
                            32.0..=4096.0,
                            self.width as f32,
                            Message::DimensionSet,
                        )
                        .style(style::Theme::Default),
                    )
                    .push(
                        Text::new(format!(
                            "Sim Width {:<5} Height {:<5} Area {}",
                            self.width,
                            self.aspect_ratio.get_height(self.width),
                            self.width * self.aspect_ratio.get_height(self.width)
                        ))
                        .size(16)
                        .vertical_alignment(VerticalAlignment::Bottom)
                        .horizontal_alignment(HorizontalAlignment::Center)
                        .width(Length::Fill),
                    );

                let load_save_column = Button::new(
                    &mut self.load_save_button,
                    Text::new("Load Save").horizontal_alignment(HorizontalAlignment::Center),
                )
                .style(style::Theme::Default)
                .min_width(style::MAIN_MENU_COLLUMN_WIDTH);

                Container::new(
                    Column::new()
                        .height(Length::Fill)
                        .width(Length::Fill)
                        .padding(60)
                        .spacing(100)
                        .align_items(Align::Center)
                        .push(Text::new("Evonomics").size(50).color(style::COLOR_GOLD))
                        .push(
                            Row::new()
                                .spacing(100)
                                .push(new_run_column)
                                .push(load_save_column),
                        ),
                )
                .style(style::Theme::Default)
                .width(Length::Fill)
                .height(Length::Fill)
                .center_x()
                .center_y()
                .into()
            }
            MenuState::SimMenu => {
                let fps_controls = Container::new(
                    Column::new()
                        .padding(style::PADDING)
                        .push(
                            Slider::new(
                                &mut self.speed_slider,
                                1.0..=100.0,
                                speed as f32,
                                Message::SpeedChanged,
                            )
                            .style(style::Theme::Default),
                        )
                        .push(
                            Text::new(format!("ticks/frame: {:<3}", speed))
                                .size(16)
                                .vertical_alignment(VerticalAlignment::Bottom)
                                .horizontal_alignment(HorizontalAlignment::Center)
                                .width(Length::Fill),
                        )
                        .push(
                            Slider::new(
                                &mut self.frame_rate_slider,
                                1.0..=32.0,
                                self.frames_per_second as f32,
                                Message::FrameRateChanged,
                            )
                            .style(style::Theme::Default),
                        )
                        .push(
                            Text::new(format!("frames/second: {:<3}", self.frames_per_second))
                                .size(16)
                                .vertical_alignment(VerticalAlignment::Bottom)
                                .horizontal_alignment(HorizontalAlignment::Center)
                                .width(Length::Fill),
                        )
                        .push(
                            Text::new(format!(
                                "ticks/second: {:.1}",
                                match &self.grid {
                                    Some(grd) => grd.get_ticks_per_second(),
                                    None => panic!("grid not set in gui thread!"),
                                }
                            ))
                            .size(16)
                            .vertical_alignment(VerticalAlignment::Bottom)
                            .horizontal_alignment(HorizontalAlignment::Center)
                            .width(Length::Fill),
                        )
                        .push(
                            Text::new(format!("Total Ticks: {}", self.total_tick_count))
                                .size(16)
                                .vertical_alignment(VerticalAlignment::Bottom)
                                .horizontal_alignment(HorizontalAlignment::Center)
                                .width(Length::Fill),
                        ),
                )
                .style(style::Theme::Nested);

                let spawn_controls = Container::new(
                    Column::new()
                        .padding(style::PADDING)
                        .push(
                            Button::new(
                                &mut self.toggle_spawn_rate_type_button,
                                Text::new(if self.is_inverse_rate_type {
                                    "Currently Dynamic"
                                } else {
                                    "Currently Constant"
                                }),
                            )
                            .style(style::Theme::Nested)
                            .width(Length::Fill)
                            .on_press(Message::ToggleRateType),
                        )
                        .push(
                            Slider::new(
                                &mut self.spawn_slider,
                                0.0..=1.0,
                                self.spawn_rate,
                                Message::SpawnRateChanged,
                            )
                            .style(style::Theme::Default),
                        )
                        .push(
                            Text::new(format!(
                                "Estimated RNG Cells/Tick\n{:.3}",
                                self.spawn_chance
                                    * self.width as f64
                                    * self.aspect_ratio.get_height(self.width) as f64
                            ))
                            .size(16)
                            .vertical_alignment(VerticalAlignment::Bottom)
                            .horizontal_alignment(HorizontalAlignment::Center)
                            .width(Length::Fill),
                        ),
                )
                .style(style::Theme::Nested);

                let grid_controls = Column::new()
                    .spacing(style::SPACING)
                    .padding(style::PADDING)
                    .max_width(style::BUTTON_SIZE + style::PADDING as u32)
                    .push(
                        Button::new(&mut self.save_simulation_button, Text::new("save"))
                            .style(style::Theme::Default)
                            .min_width(style::BUTTON_SIZE),
                    )
                    .push(
                        Button::new(
                            &mut self.toggle_run_button,
                            if self.is_running_sim {
                                Text::new("Pause")
                            } else {
                                Text::new("Run")
                            },
                        )
                        .style(style::Theme::Default)
                        .min_width(style::BUTTON_SIZE)
                        .on_press(Message::ToggleSim),
                    )
                    .push(fps_controls)
                    .push(spawn_controls)
                    .push(
                        Button::new(
                            &mut self.toggle_grid_button,
                            Text::new(match self.grid {
                                Some(ref grd) => {
                                    if grd.is_showing_lines() {
                                        "Hide Grid"
                                    } else {
                                        "Show Grid"
                                    }
                                }
                                None => panic!(
                                    "grid doesn't exist when attempting to draw grid controls!"
                                ),
                            }),
                        )
                        .style(style::Theme::Default)
                        .min_width(style::BUTTON_SIZE)
                        .on_press(Message::ToggleGrid),
                    );

                Container::new(
                    Row::new().push(
                        Row::new()
                            .push(grid_controls)
                            // TODO, .push( Text::new("Click a cell to see its genome or save it.\n\nClick an empty spot to plant a cell from the save files.\n\nUse the wheel to zoom | right click to pan.") ) )
                            //        requires tracking number of marked ancestors in EvonomicsWorld: .push( table with rows of cell ancestors, collumns of color, hide/show radio button, delete button )
                            .push(match self.grid {
                                Some(ref mut grd) => grd.view().map(|_| Message::Null),
                                None => {
                                    panic!("unexpected entry to view without initializing grid")
                                }
                            }),
                    ),
                )
                .style(style::Theme::Default)
                .width(Length::Fill)
                .height(Length::Fill)
                .center_x()
                .center_y()
                .into()
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AspectRatio {
    OneToOne,
    SixteenToTen,
}

impl AspectRatio {
    pub fn get_height(&self, width: usize) -> usize {
        match self {
            AspectRatio::OneToOne => width,
            AspectRatio::SixteenToTen => width * 5 / 8,
        }
    }
}
