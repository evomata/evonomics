mod grid;
pub mod sim;

use futures::{
    channel::mpsc::{Receiver, Sender},
    prelude::*,
};
use iced::{
    button, executor, slider, time, Align, Application, Button, Column, Command, Element,
    HorizontalAlignment, Length, Row, Settings, Slider, Space, Subscription, Text,
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
    toggle_run_button: button::State,
    toggle_grid_button: button::State,
    frame_rate_slider: slider::State,
    frames_per_second: usize,
    ms_per_frame: usize,
    speed_slider: slider::State,
    speed: usize,
    dimension_slider: slider::State,
    dimension: usize,
    menu_state: MenuState,
    is_running_sim: bool,
    next_speed: Option<usize>,
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
    DimensionSet(f32),
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
            _ => panic!("do not try to clone messages with data in them"),
        }
    }
}

fn reciever_command(rx: Receiver<sim::FromSim>) -> Command<Message> {
    Command::perform(rx.into_future(), |(item, stream)| {
        Message::FromSim(item.expect("sim_rx ended unexpectedly"), stream)
    })
}

impl<'a> Application for EvonomicsWorld {
    // application produced messages
    type Message = Message;
    // run commands and subscriptions
    type Executor = executor::Default;
    // initialization data for application
    type Flags = ();

    fn new(_: ()) -> (EvonomicsWorld, Command<Self::Message>) {
        (
            EvonomicsWorld {
                grid: None,
                sim_tx: None,
                run_simulation_button: Default::default(),
                load_save_button: Default::default(),
                save_simulation_button: Default::default(),
                toggle_run_button: Default::default(),
                toggle_grid_button: Default::default(),
                speed_slider: Default::default(),
                speed: 1,
                frame_rate_slider: Default::default(),
                frames_per_second: 1000/66,
                ms_per_frame: 66,
                dimension_slider: Default::default(),
                dimension: 512,
                menu_state: MenuState::MainMenu,
                is_running_sim: false,
                next_speed: None,
            },
            Command::none()
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
                        Some(ref mut grd) => { grd.update( view.into() ) },
                        None => {}
                    }
                }
                return reciever_command(stream);
            }
            Message::SimView => {
                self.menu_state = MenuState::SimMenu;
                self.is_running_sim = true;
                
                let (sim_tx, sim_rx, sim_runner) = sim::run_sim(2, 1, self.dimension);

                self.sim_tx = Some(sim_tx);
                self.grid = Some( grid::Grid::new( self.dimension ) );
                
                return Command::batch(
                    vec![ 
                        Command::perform( sim_runner, |_| panic!() ),
                        reciever_command(sim_rx) ] );
            }
            Message::MainView => {
                self.menu_state = MenuState::MainMenu;
                self.is_running_sim = false;
            }
            Message::FrameRateChanged(new_rate) => {
                self.frames_per_second = new_rate as usize;
                self.ms_per_frame = ( 1000.0 / new_rate ) as usize;
            }
            Message::SpeedChanged(new_speed) => {
                self.speed = new_speed as usize;
            }
            Message::DimensionSet(new_dim) => {
                self.dimension = new_dim as usize;
            }
            Message::ToggleSim => {
                self.is_running_sim = !self.is_running_sim;
            }
            Message::ToggleGrid => {
                match self.grid {
                    Some(ref mut grd) => { grd.toggle_lines() },
                    None => {}
                }
            }
            Message::Tick => {
                match self.sim_tx {
                    Some(ref mut tx) => { 
                        // If the channel is full, dont send it.
                        tx.try_send(sim::ToSim::Tick(self.speed)).ok(); 
                    },
                    None => {},
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
        const BUTTON_SIZE: u32 = 200;
        let speed = self.next_speed.unwrap_or(self.speed);
        match self.menu_state {
            MenuState::MainMenu => {
                Column::new()
                    .height(Length::Fill)
                    .width(Length::Fill)
                    .padding(100)
                    .spacing(60)
                    .align_items(Align::Center)
                    .push(Text::new("Evonomics").size(50))
                    .push( 
                        Row::new()
                        .spacing(100)
                        .push(
                            Column::new()
                                .spacing(10)
                                .max_width(BUTTON_SIZE)
                                .align_items( Align::Center )
                                .push( Button::new( &mut self.run_simulation_button, Text::new("Run Simulation").horizontal_alignment(HorizontalAlignment::Center) ).min_width(BUTTON_SIZE)
                                        .on_press(Message::SimView) 
                                )
                                .push( Slider::new( &mut self.dimension_slider, 32.0..=4096.0, self.dimension as f32, Message::DimensionSet ) )
                                .push( Text::new(format!("Sim Dimension {} (cells {})", self.dimension, self.dimension*self.dimension) ).size(16).vertical_alignment(VerticalAlignment::Bottom).horizontal_alignment(HorizontalAlignment::Center).width(Length::Fill) )
                        )
                        .push( Button::new( &mut self.load_save_button, Text::new("Load Save").horizontal_alignment(HorizontalAlignment::Center) ).min_width(BUTTON_SIZE) )
                    )
                    .into()
                // TODO: .push(settings:labels&radios&sliders) resource list with scarcity sliders, radio button for market entity, radio button for distance trading, slider for trade penalty, slider for carry capacities, slider for barter penalty
            }
            MenuState::SimMenu => {
                Row::new()
                    .push( 
                        Row::new()
                            .padding(10)
                            .push(
                                Box::new( 
                                    Column::new()
                                        .spacing(10)
                                        .max_width(220)
                                        .push( Button::new( &mut self.save_simulation_button, Text::new("save") ).min_width(BUTTON_SIZE) )
                                        .push( Button::new( &mut self.toggle_run_button, if self.is_running_sim { Text::new("Pause") } else { Text::new("Run") } ).min_width(BUTTON_SIZE)
                                            .on_press(Message::ToggleSim) )
                                )
                                .push( Slider::new( &mut self.speed_slider, 1.0..=100.0, speed as f32, Message::SpeedChanged ) )
                                .push( Slider::new( &mut self.frame_rate_slider, 1.0..=32.0, self.frames_per_second as f32, Message::FrameRateChanged ) )
                                .push( Text::new( format!("{} Ticks/frame (fps {})", speed, self.frames_per_second) ).size(16).vertical_alignment(VerticalAlignment::Bottom).horizontal_alignment(HorizontalAlignment::Center).width(Length::Fill) )
                                .push( Space::new(Length::Fill, Length::Shrink) )
                                .push( Button::new( &mut self.toggle_grid_button, Text::new("Toggle Grid") ).min_width(BUTTON_SIZE)
                                    .on_press(Message::ToggleGrid) )
                            )
                            // TODO, .push( Text::new("Click a cell to see its genome or save it.\n\nClick an empty spot to plant a cell from the save files.\n\nUse the wheel to zoom | right click to pan.") ) )
                            // TODO, requires tracking number of marked ancestors in EvonomicsWorld: .push( table with rows of cell ancestors, collumns of color, hide/show radio button, delete button )
                            .push( 
                                match self.grid {
                                    Some(ref mut grd) => {
                                        grd.view().map(|_| Message::Null) 
                                    },
                                    None => { panic!("unexpected entry to view without initializing grid") }
                                }
                            ) 
                    )
                    .into()
            }
        }
    }
}
